import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { ArrowsOutSimple } from "@phosphor-icons/react";
import { DiagramLightbox, type DiagramSize } from "@/components/markdown/DiagramLightbox";
import { diagramBounds, mermaidId, renderMermaid } from "@/components/markdown/mermaidRender";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { isDark, subscribeDark } from "@/lib/theme";
import { cn } from "@/lib/utils";
import { useScrollFade } from "@/hooks/useScrollFade";

/**
 * A ```mermaid fence, drawn. Reached from `MD_COMPONENTS.pre`, so every
 * markdown surface gets it. Drawing is `mermaidRender.ts`, shared with the
 * note editor.
 */

/** Debounce, so a streaming fence lays out once per burst, not per delta. */
const SETTLE_MS = 150;

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
  const [id] = useState(mermaidId);

  useEffect(() => {
    let live = true;
    const timer = setTimeout(async () => {
      try {
        const out = await renderMermaid(code, id);
        if (!out || !live) return;
        setSize(out.size);
        setSvg(out.svg);
      } catch {
        // Runs from a timer, so nothing may escape; keep what's on screen.
      }
    }, SETTLE_MS);
    return () => {
      live = false;
      clearTimeout(timer);
    };
    // `dark` only re-runs it: `renderMermaid` reads the theme itself.
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
                className="absolute top-1.5 right-1.5 cursor-pointer rounded-full border border-border bg-card/90 p-1.5 text-muted-foreground opacity-0 shadow-xs backdrop-blur-sm transition-opacity will-change-[opacity] hover:text-foreground focus-visible:opacity-100 group-hover:opacity-100"
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
  const bounds = natural ? (diagramBounds(natural) as React.CSSProperties) : undefined;
  const el = useRef<HTMLDivElement>(null);
  const drag = useRef<{ x: number; y: number; left: number; top: number } | null>(null);
  /** Whether the box clips the picture on either axis. */
  const [pannable, setPannable] = useState(false);

  // `> 1`, not `> 0`: a scaled width leaves sub-pixel slack that would read
  // as overflow.
  const sync = useCallback(() => {
    const box = el.current;
    if (!box) return;
    setPannable(
      box.scrollWidth - box.clientWidth > 1 || box.scrollHeight - box.clientHeight > 1,
    );
  }, []);
  useScrollFade(el, "xy", svg);

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
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      // No `bindFunctions`: under `strict` it only attaches tooltips.
      dangerouslySetInnerHTML={{ __html: svg }}
    />
  );
}
