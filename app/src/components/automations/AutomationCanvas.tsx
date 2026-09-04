import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Background,
  BackgroundVariant,
  Controls,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  useStore,
  type Connection,
  type CoordinateExtent,
  type Edge,
  type EdgeChange,
  type NodeChange,
  type NodeTypes,
  type OnConnectEnd,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import "@/components/automations/canvas.css";
import { Plus } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import CanvasNode, { type FlowNode } from "@/components/automations/CanvasNode";
import NodeInspector from "@/components/automations/NodeInspector";
import NodePalette from "@/components/automations/NodePalette";
import { type NodeSpec } from "@/components/automations/catalog";
import {
  canConnect,
  inputPorts,
  isTrigger,
  outputPorts,
  portById,
  type AutomationGraph,
  type AutomationLink,
  type AutomationNode,
  type Port,
} from "@/lib/automations";

const NODE_TYPES: NodeTypes = { automation: CanvasNode };

const COLUMN = 300;
const ROW = 130;

/** Roughly a node, for working out what the graph occupies. Ports make the
 *  real height vary; this only needs to be close. */
const NODE_W = 236;
const NODE_H = 150;

const MIN_ZOOM = 0.3;
const MAX_ZOOM = 1.5;

/** Graph left on screen at the end of a pan, in pixels. Dragging the empty
 *  pane pans the canvas, and an unbounded pan flings the whole graph out of
 *  view with no way back but the fit-view button — so panning stops while a
 *  strip of the graph is still showing. */
const PAN_KEEP_VISIBLE = 140;

/** Positions for a graph drawn before the editor existed: columns by distance
 *  from a trigger, which is the shape those linear chains already had. */
function autoLayout(g: AutomationGraph): Map<string, { x: number; y: number }> {
  const depth = new Map<string, number>();
  const queue = g.nodes.filter((n) => isTrigger(n.kind)).map((n) => n.id);
  queue.forEach((id) => depth.set(id, 0));
  while (queue.length) {
    const id = queue.shift()!;
    for (const l of g.links) {
      if (l.from !== id || depth.has(l.to)) continue;
      depth.set(l.to, (depth.get(id) ?? 0) + 1);
      queue.push(l.to);
    }
  }
  const perColumn = new Map<number, number>();
  const out = new Map<string, { x: number; y: number }>();
  for (const n of g.nodes) {
    const d = depth.get(n.id) ?? 0;
    const row = perColumn.get(d) ?? 0;
    perColumn.set(d, row + 1);
    out.set(n.id, { x: 80 + d * COLUMN, y: 80 + row * ROW });
  }
  return out;
}

/** Ports are part of a wire's identity: the same two nodes may be joined
 *  twice, once per slot. */
const edgeId = (l: AutomationLink) => `${l.from}:${l.fromPort}->${l.to}:${l.toPort}`;

/** Nudge a spot until it is not sitting on top of an existing node, so a node
 *  dropped into the middle of the view is always visibly its own. */
function unoccupied(g: AutomationGraph, at: { x: number; y: number }) {
  const spot = { ...at };
  while (g.nodes.some((n) => n.position && Math.abs(n.position.x - spot.x) < 60 && Math.abs(n.position.y - spot.y) < 60)) {
    spot.x += 40;
    spot.y += 40;
  }
  return spot;
}

/** The end of a wire let go over empty canvas: the node it came from, the port
 *  it left by, and which end of the wire that was — `source` for a drag out of
 *  an output, `target` for one dragged backwards out of an input slot. */
interface DraggedWire {
  nodeId: string;
  portId: string;
  end: "source" | "target";
}

/**
 * The port on a not-yet-created `spec` node that a wire from `port` may land
 * on — the first one `canConnect` accepts, or undefined when that kind cannot
 * take the value at all.
 *
 * The candidate is probed rather than looked up: ports are derived from a
 * node's kind and config, so the only way to know what a fresh node would
 * offer is to build one from the spec's defaults and ask. That keeps the
 * palette's idea of compatibility and the canvas's own `isValidConnection` the
 * same rule rather than two copies of one table.
 */
function matchingPort(spec: NodeSpec, port: Port, end: DraggedWire["end"]): Port | undefined {
  const probe: AutomationNode = { id: "probe", kind: spec.kind, config: { ...spec.defaults } };
  return end === "source"
    ? inputPorts(probe).find((p) => canConnect(port.type, p.type))
    : outputPorts(probe).find((p) => canConnect(p.type, port.type));
}

/**
 * The graph editor: a React Flow canvas over the stored `{nodes, links}`, with
 * a palette and an inspector for the selected node.
 *
 * Fully controlled — every drag, connection and config edit rewrites the graph
 * and hands it back, so what the canvas shows and what the executor will walk
 * are the same object. The page above debounces the write to SQLite.
 */
export default function AutomationCanvas(props: {
  graph: AutomationGraph;
  onChange: (g: AutomationGraph) => void;
}) {
  return (
    <ReactFlowProvider>
      <CanvasBody {...props} />
    </ReactFlowProvider>
  );
}

/** Drop wires whose ports no longer exist. Ports come from a node's config,
 *  so switching an event trigger from a sync to app-start, or deleting an AI
 *  node's input slot, takes the ports with it — and a wire to a port that is
 *  gone would linger in the document while drawing nothing. */
function prune(g: AutomationGraph): AutomationGraph {
  const byId = new Map(g.nodes.map((n) => [n.id, n]));
  const links = g.links.filter((l) => {
    const src = byId.get(l.from);
    const dst = byId.get(l.to);
    return (
      !!src && !!dst &&
      !!portById(outputPorts(src), l.fromPort) &&
      !!portById(inputPorts(dst), l.toPort)
    );
  });
  return links.length === g.links.length ? g : { ...g, links };
}

function CanvasBody({
  graph,
  onChange: commit,
}: {
  graph: AutomationGraph;
  onChange: (g: AutomationGraph) => void;
}) {
  const onChange = useCallback((g: AutomationGraph) => commit(prune(g)), [commit]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  /**
   * How big each node measured, kept here and handed straight back to React
   * Flow on every render.
   *
   * React Flow measures a node once, into its own copy — and throws that
   * measurement away whenever the node object it was given is replaced by a
   * different one, keeping only a `measured` the caller supplies. This canvas
   * rebuilds every node object from the graph on every change, so without this
   * the first drag frame left the node unmeasured, and an unmeasured node is
   * rendered `visibility: hidden`: the node vanished the moment you moved it
   * and never came back, because the element's own size never changed and so
   * nothing asked the resize observer to look again.
   *
   * Sizes are view state, not document state — a node's height follows from
   * its ports, so it is derived, and it must never reach the graph blob that
   * is written to SQLite.
   */
  const [measured, setMeasured] = useState<Record<string, { width: number; height: number }>>({});
  /**
   * The palette, and what picking from it should do: where to hang it (pane
   * pixels), where the node goes (flow units, or the middle of the view when
   * the toolbar button opened it), and the wire waiting to be joined up.
   */
  const [paletteOpen, setPaletteOpen] = useState(false);
  /** Consumes the single pane click that ends a wire drag. See `onConnectEnd`. */
  const keepPaletteThroughPaneClick = useRef(false);
  const [palette, setPalette] = useState<{
    at: { x: number; y: number };
    drop: { x: number; y: number } | null;
    wire: DraggedWire | null;
  }>({ at: { x: 12, y: 44 }, drop: null, wire: null });
  const pane = useRef<HTMLDivElement>(null);
  const addButton = useRef<HTMLButtonElement>(null);
  const { screenToFlowPosition } = useReactFlow();

  const layout = useMemo(() => autoLayout(graph), [graph]);

  // React Flow measures the pane for us; the margin below is in flow units,
  // which is what the extent is expressed in.
  const paneW = useStore((s) => s.width);
  const paneH = useStore((s) => s.height);

  /**
   * The pannable area: the graph's own bounds plus a margin, recomputed as
   * nodes move — so dragging a node outwards extends where you may pan rather
   * than fencing it in.
   *
   * The margin is one viewport minus the strip that must stay visible, taken
   * at the tightest zoom, because that is where the viewport covers the fewest
   * flow units: keep the graph on screen there and it is on screen at every
   * other zoom too.
   */
  const translateExtent = useMemo<CoordinateExtent>(() => {
    const marginX = Math.max(200, paneW / MAX_ZOOM - PAN_KEEP_VISIBLE);
    const marginY = Math.max(160, paneH / MAX_ZOOM - PAN_KEEP_VISIBLE);
    const spots = graph.nodes.map((n) => n.position ?? layout.get(n.id) ?? { x: 80, y: 80 });
    if (spots.length === 0) {
      return [
        [-marginX, -marginY],
        [marginX, marginY],
      ];
    }
    const xs = spots.map((p) => p.x);
    const ys = spots.map((p) => p.y);
    return [
      [Math.min(...xs) - marginX, Math.min(...ys) - marginY],
      [Math.max(...xs) + NODE_W + marginX, Math.max(...ys) + NODE_H + marginY],
    ];
  }, [graph, layout, paneW, paneH]);

  const nodes: FlowNode[] = useMemo(
    () =>
      graph.nodes.map((n) => ({
        id: n.id,
        type: "automation" as const,
        position: n.position ?? layout.get(n.id) ?? { x: 80, y: 80 },
        data: { node: n },
        selected: n.id === selectedId,
        // A node we have never measured gets the standard box rather than
        // nothing. React Flow hides a node it considers unmeasured, and a node
        // added after mount was observed to never receive its `dimensions`
        // event at all — so "no measurement yet" must not mean "invisible".
        // The real size replaces this the moment one arrives.
        measured: measured[n.id] ?? { width: NODE_W, height: NODE_H },
      })),
    [graph, layout, selectedId, measured],
  );

  const edges: Edge[] = useMemo(
    () =>
      graph.links.map((l) => {
        const src = graph.nodes.find((n) => n.id === l.from);
        const type = src ? portById(outputPorts(src), l.fromPort)?.type : undefined;
        return {
          id: edgeId(l),
          source: l.from,
          target: l.to,
          sourceHandle: l.fromPort,
          targetHandle: l.toPort,
          type: "smoothstep",
          className:
            l.fromPort === "true" && src?.kind === "condition.if"
              ? "branch-true"
              : type && type !== "signal"
                ? "data-wire"
                : undefined,
        };
      }),
    [graph],
  );

  const onNodesChange = useCallback(
    (changes: NodeChange<FlowNode>[]) => {
      let next = graph;
      for (const ch of changes) {
        if (ch.type === "position" && ch.position) {
          const pos = ch.position;
          next = {
            ...next,
            nodes: next.nodes.map((n) => (n.id === ch.id ? { ...n, position: pos } : n)),
          };
        } else if (ch.type === "remove") {
          next = {
            nodes: next.nodes.filter((n) => n.id !== ch.id),
            links: next.links.filter((l) => l.from !== ch.id && l.to !== ch.id),
          };
          setSelectedId((cur) => (cur === ch.id ? null : cur));
          setMeasured(({ [ch.id]: _gone, ...rest }) => rest);
        } else if (ch.type === "select" && ch.selected) {
          setSelectedId(ch.id);
        } else if (ch.type === "dimensions" && ch.dimensions) {
          const size = ch.dimensions;
          setMeasured((cur) =>
            cur[ch.id]?.width === size.width && cur[ch.id]?.height === size.height
              ? cur
              : { ...cur, [ch.id]: size },
          );
        }
      }
      if (next !== graph) onChange(next);
    },
    [graph, onChange],
  );

  const onEdgesChange = useCallback(
    (changes: EdgeChange<Edge>[]) => {
      const removed = changes.filter((c) => c.type === "remove").map((c) => c.id);
      if (removed.length === 0) return;
      onChange({
        ...graph,
        links: graph.links.filter((l) => !removed.includes(edgeId(l))),
      });
    },
    [graph, onChange],
  );

  /** A wire is legal when the value on the output port can be read by the
   *  slot it lands in — everything reads as text or as a bare "run after
   *  this", but a file list cannot be conjured from a sentence. */
  const isValidConnection = useCallback(
    (c: Connection | Edge) => {
      if (!c.source || !c.target || c.source === c.target) return false;
      const src = graph.nodes.find((n) => n.id === c.source);
      const dst = graph.nodes.find((n) => n.id === c.target);
      if (!src || !dst) return false;
      const from = portById(outputPorts(src), c.sourceHandle ?? "");
      const to = portById(inputPorts(dst), c.targetHandle ?? "");
      return !!from && !!to && canConnect(from.type, to.type);
    },
    [graph],
  );

  const onConnect = useCallback(
    (c: Connection) => {
      if (!isValidConnection(c) || !c.sourceHandle || !c.targetHandle) return;
      const link: AutomationLink = {
        from: c.source,
        fromPort: c.sourceHandle,
        to: c.target,
        toPort: c.targetHandle,
      };
      if (graph.links.some((l) => edgeId(l) === edgeId(link))) return;
      onChange({ ...graph, links: [...graph.links, link] });
    },
    [graph, isValidConnection, onChange],
  );

  /** Open the palette under the toolbar button, with no wire waiting. */
  const openPalette = useCallback(() => {
    const b = addButton.current?.getBoundingClientRect();
    const p = pane.current?.getBoundingClientRect();
    setPalette({
      at: b && p ? { x: b.left - p.left, y: b.bottom - p.top + 4 } : { x: 12, y: 44 },
      drop: null,
      wire: null,
    });
    setPaletteOpen(true);
  }, []);

  /**
   * A wire let go over empty canvas asks what should be on the other end: the
   * palette opens where you dropped it, narrowed to the kinds that can take
   * the value, and picking one both creates the node and draws the wire.
   *
   * Releases that landed on a handle are React Flow's business — a valid one
   * has already gone through `onConnect`, and an invalid one is a refusal, not
   * a request for a new node.
   */
  const onConnectEnd = useCallback<OnConnectEnd>(
    (event, state) => {
      if (!state.fromHandle || state.toHandle || state.toNode) return;
      const point = "changedTouches" in event ? event.changedTouches[0] : event;
      const p = pane.current?.getBoundingClientRect();
      setPalette({
        at: p ? { x: point.clientX - p.left, y: point.clientY - p.top } : { x: 12, y: 44 },
        drop: screenToFlowPosition({ x: point.clientX, y: point.clientY }),
        wire: {
          nodeId: state.fromHandle.nodeId,
          portId: state.fromHandle.id ?? "",
          end: state.fromHandle.type,
        },
      });
      setPaletteOpen(true);
      // Letting go of the wire is also a click on the pane, and that click
      // arrives after this — so without a one-shot pass the palette would be
      // dismissed by the very gesture that asked for it.
      keepPaletteThroughPaneClick.current = true;
    },
    [screenToFlowPosition],
  );

  /** The port a pending wire left by, resolved against the live graph. */
  const draggedPort = useMemo<Port | undefined>(() => {
    const wire = palette.wire;
    if (!wire) return undefined;
    const from = graph.nodes.find((n) => n.id === wire.nodeId);
    if (!from) return undefined;
    return portById(wire.end === "source" ? outputPorts(from) : inputPorts(from), wire.portId);
  }, [palette.wire, graph]);

  const accepts = useMemo(
    () =>
      draggedPort && palette.wire
        ? (spec: NodeSpec) => !!matchingPort(spec, draggedPort, palette.wire!.end)
        : undefined,
    [draggedPort, palette.wire],
  );

  const addNode = (spec: NodeSpec) => {
    // Dropped where the wire was let go, or — from the toolbar button — into
    // the middle of whatever the user is looking at, since a node that lands
    // off-screen reads as nothing having happened.
    const box = pane.current?.getBoundingClientRect();
    const centre = box
      ? screenToFlowPosition({ x: box.left + box.width / 2, y: box.top + box.height / 2 })
      : { x: 80, y: 80 };
    const wire = palette.wire;
    // On a wire drop the edge the wire lands on goes under the cursor, so a
    // node fetched by dragging backwards out of an input sits to its left.
    const origin = palette.drop
      ? {
          x: wire?.end === "target" ? palette.drop.x - NODE_W : palette.drop.x,
          y: palette.drop.y - 32,
        }
      : { x: centre.x - NODE_W / 2, y: centre.y - 32 };

    const id = `n${Date.now().toString(36)}`;
    const node: AutomationNode = {
      id,
      kind: spec.kind,
      config: { ...spec.defaults },
      position: unoccupied(graph, origin),
    };

    const landing = wire && draggedPort ? matchingPort(spec, draggedPort, wire.end) : undefined;
    const link: AutomationLink | undefined =
      wire && draggedPort && landing
        ? wire.end === "source"
          ? { from: wire.nodeId, fromPort: draggedPort.id, to: id, toPort: landing.id }
          : { from: id, fromPort: landing.id, to: wire.nodeId, toPort: draggedPort.id }
        : undefined;

    onChange({
      nodes: [...graph.nodes, node],
      links: link ? [...graph.links, link] : graph.links,
    });
    setSelectedId(id);
    setPaletteOpen(false);
  };

  // ⌘K reaches the palette without the mouse. AppLayout owns ⌘\ and the zoom
  // keys, and this listener lives and dies with the editor page.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.key.toLowerCase() !== "k") return;
      e.preventDefault();
      openPalette();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [openPalette]);

  const selected = graph.nodes.find((n) => n.id === selectedId) ?? null;

  return (
    <div className="flex h-full min-h-0 flex-1">
      <div ref={pane} className="oculus-canvas relative min-w-0 flex-1">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={NODE_TYPES}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          onConnectEnd={onConnectEnd}
          isValidConnection={isValidConnection}
          /* The palette is dismissed from here rather than left to Radix.
             Measured: Escape closes it, but a click on the canvas does not —
             React Flow's pane runs on d3-zoom, which swallows the mouse events
             Radix's outside-detection is watching for, so the popup would sit
             there while you carried on drawing underneath it. Closing on the
             canvas's own callbacks is one rule that cannot come apart. */
          onPaneClick={() => {
            setSelectedId(null);
            if (keepPaletteThroughPaneClick.current) {
              keepPaletteThroughPaneClick.current = false;
              return;
            }
            setPaletteOpen(false);
          }}
          onNodeClick={() => setPaletteOpen(false)}
          onMoveStart={() => setPaletteOpen(false)}
          fitView
          fitViewOptions={{ padding: 0.3, maxZoom: 1 }}
          translateExtent={translateExtent}
          minZoom={MIN_ZOOM}
          maxZoom={MAX_ZOOM}
          proOptions={{ hideAttribution: true }}
          deleteKeyCode={["Backspace", "Delete"]}
        >
          <Background variant={BackgroundVariant.Dots} gap={16} size={1} color="var(--color-border)" />
          <Controls showInteractive={false} position="bottom-right" />
        </ReactFlow>

        <div className="absolute left-3 top-3 z-10">
          <Button
            ref={addButton}
            size="sm"
            variant="outline"
            className="h-7 bg-card shadow-xs"
            onClick={openPalette}
          >
            <Plus size={13} /> Add node
          </Button>
        </div>

        <NodePalette
          open={paletteOpen}
          onOpenChange={setPaletteOpen}
          at={palette.at}
          accepts={accepts}
          onPick={addNode}
        />
      </div>

      {selected && (
        <NodeInspector
          node={selected}
          graph={graph}
          onChange={(config) =>
            onChange({
              ...graph,
              nodes: graph.nodes.map((n) => (n.id === selected.id ? { ...n, config } : n)),
            })
          }
          onDelete={() => {
            onChange({
              nodes: graph.nodes.filter((n) => n.id !== selected.id),
              links: graph.links.filter((l) => l.from !== selected.id && l.to !== selected.id),
            });
            setSelectedId(null);
          }}
        />
      )}
    </div>
  );
}
