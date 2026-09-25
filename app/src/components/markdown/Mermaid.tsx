import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import type { MermaidConfig } from "mermaid";
import { ArrowsOutSimple } from "@phosphor-icons/react";
import { DiagramLightbox, type DiagramSize } from "@/components/markdown/DiagramLightbox";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { isDark, subscribeDark } from "@/lib/theme";
import { cn } from "@/lib/utils";

/**
 * A ```mermaid fence, drawn. Reached from `MD_COMPONENTS.pre`, so every
 * markdown surface gets it; nothing above this file knows mermaid exists.
 */

// ── Loading ──────────────────────────────────────────────────────────────────

/** Mermaid is large and rarely needed, so it loads lazily on the first fence;
 *  the promise is shared. */
let loading: Promise<typeof import("mermaid").default> | null = null;

function load() {
  loading ??= import("mermaid").then((m) => m.default);
  return loading;
}

// ── Theme ────────────────────────────────────────────────────────────────────

/**
 * The `--diagram-*` palette (`index.css`) resolved to real values: mermaid
 * derives shades from them and rejects `var(...)`. Read per render because
 * `.dark` changes them; the fallbacks exist because one empty token throws out
 * of the whole render.
 */
function palette() {
  const style = getComputedStyle(document.documentElement);
  const token = (name: string, fallback: string) =>
    style.getPropertyValue(name).trim() || fallback;
  return {
    node: token("--diagram-node", "#f4f4f4"),
    nodeAlt: token("--diagram-node-alt", "#ececec"),
    ground: token("--diagram-ground", "#f9f9f9"),
    paper: token("--diagram-paper", "#ffffff"),
    border: token("--diagram-border", "#e5e5e5"),
    stroke: token("--diagram-stroke", "#b4b4b4"),
    edge: token("--diagram-edge", "#757575"),
    ink: token("--diagram-ink", "#0d0d0d"),
    font: token("--diagram-font", "Inter, system-ui, sans-serif"),
    series: [1, 2, 3, 4, 5].map((n, i) =>
      token(`--diagram-series-${n}`, SERIES_FALLBACK[i]),
    ),
  };
}

const SERIES_FALLBACK = ["#5e6ad2", "#1baf7a", "#eda100", "#e87ba4", "#008300"];

/**
 * The label size mermaid draws at, and the smallest it may be rendered at.
 * Mermaid fits a diagram to its column, so a wide one would shrink its labels
 * unreadably; instead `.diagram` (`index.css`) clamps the rendered width so the
 * scale never drops below `MIN_LABEL_PX / LABEL_PX`, and overflow scrolls.
 */
const LABEL_PX = 13;
const MIN_LABEL_PX = 11;

/** Height a tall diagram shrinks towards, within the label band above (not a
 *  hard cap — `.diagram` has one). The knob for "diagrams feel too big". */
const TARGET_HEIGHT_PX = 480;

/**
 * Flowchart spacing, ~half mermaid's defaults, to shrink tall graphs without
 * shrinking their labels. `layout` must accompany `rankSpacing`: with
 * top-level `htmlLabels: false`, mermaid 12 ignores `rankSpacing`/`nodeSpacing`
 * unless `layout` is set (see `config`).
 */
const FLOWCHART_LAYOUT = {
  rankSpacing: 24,
  nodeSpacing: 32,
  padding: 6,
} as const;

/** Mermaid's categorical slots from the chart palette (otherwise they derive
 *  from the grey `primaryColor`). Cycled in order — the order is part of the
 *  palette's validation. `pie` counts from 1, `cScale`/`git` from 0. */
function categorical(series: string[], ink: string) {
  const vars: Record<string, string> = {};
  for (let i = 0; i < 12; i++) {
    const colour = series[i % series.length];
    if (i < 8) vars[`git${i}`] = colour;
    vars[`pie${i + 1}`] = colour;
    vars[`cScale${i}`] = colour;
    // Not theme ink: the label sits on a mid-lightness series colour in both
    // themes, where only a dark ink reads.
    vars[`cScaleLabel${i}`] = ink;
  }
  return vars;
}

/**
 * Mermaid's `base` theme in the app's neutral palette (the indigo is kept for
 * controls). `darkMode` steers mermaid's shade derivation — unset, a dark
 * palette gets near-black text on near-black fills.
 */
function config(dark: boolean): MermaidConfig {
  const t = palette();
  return {
    startOnLoad: false,
    // Required for `FLOWCHART_LAYOUT`'s spacing to apply.
    layout: "dagre",
    // Fences come from a model or a PDF parser: untrusted.
    securityLevel: "strict",
    // On failure the component shows the source instead of mermaid's red card.
    suppressErrorRendering: true,
    theme: "base",
    fontFamily: t.font,
    fontSize: LABEL_PX,
    // SVG `<text>` labels; only the top-level key takes effect (mermaid 12
    // ignores the `flowchart` one). HTML labels decide wrapping by an exact
    // float compare against `getBoundingClientRect`, which the app's page zoom
    // breaks, so at zoom ≠ 1 every long label is clipped.
    htmlLabels: false,
    flowchart: { htmlLabels: false, ...FLOWCHART_LAYOUT },
    themeVariables: {
      darkMode: dark,
      fontFamily: t.font,
      fontSize: `${LABEL_PX}px`,
      background: t.paper,
      // In the `base` theme `primaryColor` is the node fill, not an accent.
      primaryColor: t.node,
      primaryTextColor: t.ink,
      primaryBorderColor: t.stroke,
      secondaryColor: t.nodeAlt,
      tertiaryColor: t.ground,
      mainBkg: t.node,
      nodeBorder: t.stroke,
      lineColor: t.edge,
      textColor: t.ink,
      clusterBkg: t.ground,
      clusterBorder: t.border,
      titleColor: t.ink,
      // These stay highlighter yellow unless set.
      edgeLabelBackground: t.paper,
      noteBkgColor: t.nodeAlt,
      noteTextColor: t.ink,
      noteBorderColor: t.border,
      ...categorical(t.series, SERIES_INK),
      // Percentages sit on a slice (series ink); title and legend on the card.
      pieSectionTextColor: SERIES_INK,
      pieTitleTextColor: t.ink,
      pieLegendTextColor: t.ink,
      pieStrokeColor: t.paper,
      pieOuterStrokeColor: t.paper,
    },
  };
}

/** See [`categorical`]: the one ink that reads on every series colour. */
const SERIES_INK = "#101010";

// ── The component ────────────────────────────────────────────────────────────

/** Per-diagram ids: mermaid scopes the SVG's `<style>` and `<marker>`s by it. */
let seq = 0;

/** Debounce, so a streaming fence lays out once per burst, not per delta. */
const SETTLE_MS = 150;

/** The drawn size, read from the SVG's `viewBox`; falls back to its inline
 *  `max-width` (square guess) for types that emit no viewBox. */
function naturalSize(svg: string): DiagramSize | null {
  const box = /viewBox="\s*[\d.-]+\s+[\d.-]+\s+([\d.]+)\s+([\d.]+)/.exec(svg);
  if (box) return { width: Number(box[1]), height: Number(box[2]) };
  const capped = /max-width:\s*([\d.]+)px/.exec(svg);
  return capped ? { width: Number(capped[1]), height: Number(capped[1]) } : null;
}

/** The SVG with its ids renamed, for the lightbox copy — duplicate ids would
 *  point its arrowheads and styles at the inline copy. The id is unique enough
 *  for a blind replace. */
function rescope(svg: string, id: string): string {
  // Not `replaceAll`: the TS lib target is below ES2021.
  return svg.split(id).join(`${id}-open`);
}

export function Mermaid({
  code,
  className,
  children,
}: {
  code: string;
  className?: string;
  /** The fence as a code block, shown until (or instead of) the diagram. */
  children: React.ReactNode;
}) {
  const dark = useSyncExternalStore(subscribeDark, isDark);
  const [svg, setSvg] = useState<string | null>(null);
  const [size, setSize] = useState<DiagramSize | null>(null);
  const [open, setOpen] = useState(false);
  const [id] = useState(() => `oculus-mermaid-${++seq}`);

  useEffect(() => {
    let live = true;
    const timer = setTimeout(async () => {
      try {
        const mermaid = await load();
        if (!live) return;
        // Global config, re-applied per render (the theme may have flipped).
        // Inside the `try`: a bad colour throws from here.
        mermaid.initialize(config(dark));
        // A half-written fence parses to `false` quietly instead of throwing.
        if (!(await mermaid.parse(code, { suppressErrors: true })) || !live) return;
        const out = await mermaid.render(id, code);
        if (!live) return;
        setSize(naturalSize(out.svg));
        setSvg(out.svg);
      } catch {
        // Runs from a timer, so nothing may escape; keep what's on screen.
      }
    }, SETTLE_MS);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [code, dark, id]);

  if (svg == null) return <>{children}</>;

  return (
    <figure
      // `selectionMarkdown` copies the fence from here, not the SVG's labels.
      data-md={`\`\`\`mermaid\n${code}\n\`\`\``}
      className={cn("diagram-figure group relative", className)}
    >
      <Scroller svg={svg} natural={size} />
      {size && (
        <>
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                onClick={() => setOpen(true)}
                aria-label="Open diagram"
                // The only way into the lightbox: a press on the picture pans.
                className="absolute top-1.5 right-1.5 cursor-pointer rounded-full border border-border bg-card/90 p-1.5 text-muted-foreground opacity-0 shadow-xs backdrop-blur-sm transition-opacity hover:text-foreground focus-visible:opacity-100 group-hover:opacity-100"
              >
                <ArrowsOutSimple size={13} />
              </button>
            </TooltipTrigger>
            <TooltipContent>Open diagram</TooltipContent>
          </Tooltip>
          <DiagramLightbox
            svg={rescope(svg, id)}
            size={size}
            open={open}
            onOpenChange={setOpen}
          />
        </>
      )}
    </figure>
  );
}

/** The inline picture, fitted to the column and scrolling past `.diagram`'s
 *  bounds. A press pans (only when there is overflow); it never opens the
 *  lightbox, so labels stay selectable. */
function Scroller({ svg, natural }: { svg: string; natural: DiagramSize | null }) {
  /**
   * The widths `.diagram` clamps `100%` between: as large as possible without
   * passing the natural size or `TARGET_HEIGHT_PX`, never so small a label
   * drops under `MIN_LABEL_PX` (`--diagram-min`; past it, overflow).
   */
  const bounds = natural
    ? (() => {
        const floor = MIN_LABEL_PX / LABEL_PX;
        const ceiling = Math.min(1, Math.max(TARGET_HEIGHT_PX / natural.height, floor));
        return {
          "--diagram-max": `${natural.width * ceiling}px`,
          "--diagram-min": `${natural.width * floor}px`,
        } as React.CSSProperties;
      })()
    : undefined;
  const el = useRef<HTMLDivElement>(null);
  const drag = useRef<{ x: number; y: number; left: number; top: number } | null>(null);
  /** Whether the box clips the picture on either axis. */
  const [pannable, setPannable] = useState(false);

  /**
   * Set `.diagram`'s per-edge fade vars (written to the element, not state —
   * this runs per scroll event) and `pannable`. `> 1`, not `> 0`: a scaled
   * width leaves sub-pixel slack that would read as overflow.
   */
  const sync = useCallback(() => {
    const box = el.current;
    if (!box) return;
    const overX = box.scrollWidth - box.clientWidth;
    const overY = box.scrollHeight - box.clientHeight;
    setPannable(overX > 1 || overY > 1);
    const edge = (on: boolean) => (on ? "1" : "0");
    box.style.setProperty("--fade-inline-start", edge(box.scrollLeft > 1));
    box.style.setProperty("--fade-inline-end", edge(box.scrollLeft < overX - 1));
    box.style.setProperty("--fade-block-start", edge(box.scrollTop > 1));
    box.style.setProperty("--fade-block-end", edge(box.scrollTop < overY - 1));
  }, []);

  useLayoutEffect(() => {
    const box = el.current;
    if (!box) return;
    sync();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(sync);
    ro.observe(box);
    return () => ro.disconnect();
  }, [svg, sync]);

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0 || !el.current || !pannable) return;
    drag.current = {
      x: e.clientX,
      y: e.clientY,
      left: el.current.scrollLeft,
      top: el.current.scrollTop,
    };
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current;
    if (!d || !el.current) return;
    const dx = e.clientX - d.x;
    const dy = e.clientY - d.y;
    if (!el.current.hasPointerCapture(e.pointerId)) {
      // Capture only once it's a drag, so a plain press stays the page's.
      if (Math.abs(dx) < 4 && Math.abs(dy) < 4) return;
      el.current.setPointerCapture(e.pointerId);
    }
    el.current.scrollLeft = d.left - dx;
    el.current.scrollTop = d.top - dy;
  };
  const endDrag = (e: React.PointerEvent) => {
    drag.current = null;
    const box = e.currentTarget as HTMLElement;
    if (box.hasPointerCapture(e.pointerId)) box.releasePointerCapture(e.pointerId);
  };

  return (
    <div
      ref={el}
      className={cn("diagram", pannable && "cursor-grab active:cursor-grabbing")}
      style={bounds}
      role="img"
      onScroll={sync}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      // No `bindFunctions`: under `strict` it only attaches tooltips.
      dangerouslySetInnerHTML={{ __html: svg }}
    />
  );
}
