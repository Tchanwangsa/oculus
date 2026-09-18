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
 * A ```mermaid fence, drawn.
 *
 * Reached from `MD_COMPONENTS.pre` (`MdComponents.tsx`), so every markdown
 * surface in the app gets it at once — the chat timeline, a parsed course
 * file in the viewer, a calendar popover. The fence is the whole API: nothing
 * above this file knows mermaid exists.
 */

// ── Loading ──────────────────────────────────────────────────────────────────

/** Mermaid is ~1 MB of parsers and layout engines and most sessions never open
 *  a diagram, so it is imported on first sight of a fence rather than with the
 *  bundle. Vite splits it out on the dynamic import; the promise is cached
 *  here so a reply with four diagrams in it still fetches once. */
let loading: Promise<typeof import("mermaid").default> | null = null;

function load() {
  loading ??= import("mermaid").then((m) => m.default);
  return loading;
}

// ── Theme ────────────────────────────────────────────────────────────────────

/**
 * The diagram palette, resolved to real values.
 *
 * Mermaid derives a dozen shades from each colour it is given, so it needs
 * values rather than names — a `var(--diagram-node)` handed to it comes back
 * as "Unsupported color format" and no diagram at all. Read fresh on each
 * render rather than cached, because they change under us when `.dark` is
 * toggled.
 *
 * The `--diagram-*` aliases are declared in `index.css`, which is also where
 * the note lives on why the component reads those and not the palette tokens
 * directly. The fallbacks are for the failure that note describes: an empty
 * string is the shape a missing token arrives in, and one of them throws out
 * of the whole render, so a neutral grey is a better answer than no picture.
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
 * What mermaid draws a label at, and the smallest that label may be *rendered*
 * at — the pair is the whole sizing policy, so both live here.
 *
 * Mermaid's own sizing is `width: 100%` under an inline `max-width` of the
 * diagram's natural width, which fits the picture to whatever it is dropped
 * into. The consequence is that the column, not the design, decides how big
 * the type is: measured in the 760px chat page, the twelve-node `graph TD`
 * (natural 360 × 791) comes out at scale 1.0 with 13px labels while a
 * five-column `graph LR` (natural 1525 × 181) is fitted to 0.50 and its labels
 * land at **6.5px**. Two diagrams in one reply, one at body-text size and one
 * an unreadable smear, and nothing in the fence to explain why.
 *
 * So the *label* is what gets bounded rather than the box. `.diagram` in
 * `index.css` clamps the rendered width so the scale can never fall below
 * `MIN_LABEL_PX / LABEL_PX`, and a diagram that needs more room than that
 * overflows and scrolls instead of shrinking. The band is [11px, 13px]: 13 is
 * the app's body text less a notch, which is the size a diagram drawn for this
 * page should be, and 11px is already the app's smallest real text (the
 * chapter-summary rule above it in `index.css`), so nothing here is smaller
 * than something the app already asks people to read.
 *
 * Bounding the label bounds the shapes with it, which is why there is no
 * separate cap on a node: mermaid sizes every box from its own label and its
 * padding, so a ceiling on the type is a ceiling on the box.
 */
const LABEL_PX = 13;
const MIN_LABEL_PX = 11;

/**
 * The height a figure aims to come in under, in px, before the band above is
 * spent shrinking it.
 *
 * Not a hard cap — `.diagram` has one of those. This is the number that decides
 * how much of the [11px, 13px] range a *tall* diagram is allowed to spend: a
 * short one is drawn at 13px because it already fits, and a tall one walks down
 * the band until it either fits or runs out of band. It is the one knob for
 * "diagrams feel too big / too small" and nothing else reads it.
 */
const TARGET_HEIGHT_PX = 480;

/**
 * Flowchart spacing, and `layout` is the load-bearing key.
 *
 * Mermaid's default rank spacing is where a `graph TD` gets its size: measured
 * on the twelve-node flowchart, its 791px is 481px of node boxes (165 of which
 * is one decision diamond) and **310px of gaps** — seven ranks at ~44px. Which
 * makes `rankSpacing` the one lever that shrinks a diagram without touching the
 * size of anything in it.
 *
 * It does nothing on its own. With the top-level `htmlLabels: false` this file
 * depends on, `flowchart.rankSpacing` and `nodeSpacing` are read and discarded
 * — measured on mermaid 12.0.0, byte-identical output at 0, at 20 and at 200,
 * while `padding` in the same object applies. Naming `layout` routes the
 * flowchart through the pluggable layout loader instead, where they take
 * effect. So the two keys ship together; drop `layout` and the spacing below
 * silently stops meaning anything.
 *
 * The values are ~half of mermaid's defaults, measured on the same flowchart:
 * 791 × 360 becomes 661 × 352, and a five-column `graph LR` 1450 wide becomes
 * 1266. `layout: "dagre"` is flowchart-only — sequence and pie render
 * byte-identically with it set — and labels stay SVG `<text>`, which is the
 * thing that must not regress (see `htmlLabels` below).
 */
const FLOWCHART_LAYOUT = {
  rankSpacing: 24,
  nodeSpacing: 32,
  padding: 6,
} as const;

/** Mermaid's categorical slots, filled from the app's chart palette.
 *
 *  Without this a pie or a journey comes out as twelve shades derived from
 *  `primaryColor`, which here is a grey — measured, the slices of a five-part
 *  pie were not distinguishable from each other. The palette in `index.css`
 *  is already validated for CVD separation and for contrast against the card,
 *  and its slot order is part of that, so it is cycled rather than reordered.
 *
 *  `pie1…` is the pie's own family; `cScale0…` is what journey, timeline and
 *  quadrant read; `git0…` is gitGraph's. Mermaid numbers the first from 1 and
 *  the other two from 0. */
function categorical(series: string[], ink: string) {
  const vars: Record<string, string> = {};
  for (let i = 0; i < 12; i++) {
    const colour = series[i % series.length];
    if (i < 8) vars[`git${i}`] = colour;
    vars[`pie${i + 1}`] = colour;
    vars[`cScale${i}`] = colour;
    // One ink for every slot, and deliberately not `--color-foreground`: the
    // label sits on the *series colour*, which is the same mid-lightness band
    // in both themes, so an ink that flipped with the theme would be white on
    // amber half the time. Dark reads on all five in both palettes.
    vars[`cScaleLabel${i}`] = ink;
  }
  return vars;
}

/**
 * Mermaid's `base` theme with this app's palette poured into it.
 *
 * Neutral on purpose: the design direction gives the app one colour and the
 * indigo is spent on controls, so a diagram that reached for it would read as
 * a second accent competing with the button beside it. Greys and one ink
 * weight is also what the tables in this app already look like.
 *
 * `darkMode` is not decoration — it tells mermaid's own derivation whether to
 * lighten or darken the shades it builds from these, so a dark palette with
 * the flag unset comes out with near-black text on near-black fills.
 */
function config(dark: boolean): MermaidConfig {
  const t = palette();
  return {
    startOnLoad: false,
    // See `FLOWCHART_LAYOUT`: without this the spacing there is discarded.
    layout: "dagre",
    // The fence is written by a model or by a PDF parser, so labels are
    // untrusted text: `strict` encodes any HTML in them and refuses the
    // `click` directive's handlers outright.
    securityLevel: "strict",
    // A diagram that fails to render must not paint mermaid's own red error
    // card into the reply — this component falls back to showing the source,
    // which is what an unrecognised fence did before it existed.
    suppressErrorRendering: true,
    theme: "base",
    fontFamily: t.font,
    fontSize: LABEL_PX,
    // **Labels as SVG `<text>`, and this key has to be at the top level.**
    // Under `flowchart` it is silently ignored (measured, mermaid 12.0: still
    // four `<foreignObject>`s and not one `<text>` label); at the top level
    // mermaid propagates it and the HTML label path never runs. Both are set
    // because the nested one is the documented spelling and costs nothing, but
    // the outer one is the one doing the work.
    //
    // It is not cosmetic. Mermaid lays an HTML label out by setting
    // `white-space: nowrap` with a `max-width`, measuring the result, and
    // switching to wrapping only when `bbox.width === width` — an exact float
    // equality against the unscaled constant it just set. **This app runs the
    // webview at a page zoom** (`setZoom`, `AppLayout`), which scales
    // `getBoundingClientRect`, so at any zoom but 1 that equality is false,
    // the label never wraps, and every caption longer than the box is cut off
    // mid-word — with the edge labels' backgrounds mis-sized to match, so the
    // connector line draws straight through their text. Measured at zoom 1.3:
    // four labels, four clipped. An SVG `<text>` label is laid out by mermaid
    // itself and has neither problem at any zoom.
    //
    // It also settles a second question for free: an SVG label cannot be
    // reached by `.md-compact`'s markdown rules at all. The `.diagram` guard in
    // `index.css` stays for the diagram types that still use HTML labels.
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
      // Three that stay a highlighter yellow however the rest is themed.
      edgeLabelBackground: t.paper,
      noteBkgColor: t.nodeAlt,
      noteTextColor: t.ink,
      noteBorderColor: t.border,
      ...categorical(t.series, SERIES_INK),
      // The pie's own labels: the percentage sits on a slice and takes the
      // series ink, while the title and legend sit on the card and follow the
      // theme like any other text. Slices are separated by the card colour
      // rather than by a stroke of their own.
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

/** Each diagram needs an id of its own: mermaid scopes the `<style>` and the
 *  arrowhead `<marker>` definitions it puts *inside* the SVG by it, so two
 *  diagrams sharing one id would have the second's arrows point at the
 *  first's markers. */
let seq = 0;

/** A reply streams in a character at a time, so this component sees a fence
 *  that is still being written — `graph TD` before the arrows, a node with no
 *  closing bracket. Re-laying out a graph on every keystroke is the expensive
 *  end of this library, so a burst of deltas collapses into one render. */
const SETTLE_MS = 150;

/** How big the diagram mermaid drew wants to be, in px.
 *
 *  Mermaid does not report it, but it writes it into the SVG twice over: the
 *  `viewBox` is the drawing's own extent, and with its default `useMaxWidth`
 *  the element comes out as `width="100%"` capped by an inline `max-width` of
 *  the same number. The viewBox is the one to read — it carries the height
 *  too, which the lightbox needs to fit the picture to a window. The
 *  `max-width` is the fallback for a diagram type that emits no viewBox, and
 *  then the height is guessed from the element's own aspect ratio. */
function naturalSize(svg: string): DiagramSize | null {
  const box = /viewBox="\s*[\d.-]+\s+[\d.-]+\s+([\d.]+)\s+([\d.]+)/.exec(svg);
  if (box) return { width: Number(box[1]), height: Number(box[2]) };
  const capped = /max-width:\s*([\d.]+)px/.exec(svg);
  return capped ? { width: Number(capped[1]), height: Number(capped[1]) } : null;
}

/** The same SVG with its ids moved out of the way.
 *
 *  A diagram that is open in the lightbox is still on screen in the reply
 *  behind it, so the document holds two copies of markup mermaid scoped to
 *  one id — the `<style>` block it puts *inside* the SVG is written as
 *  `#<id> .node {…}`, and arrowheads are `url(#<id>-…)` references. Duplicate
 *  ids resolve to whichever comes first in the document, so without this the
 *  lightbox's arrows would point at markers belonging to the small copy.
 *  The id is a token nothing else in the file can match, so a blind replace
 *  is exactly right here. */
function rescope(svg: string, id: string): string {
  // `split`/`join` rather than `replaceAll`: this project's TS lib target is
  // below ES2021, and the one-line convenience is not worth widening it.
  return svg.split(id).join(`${id}-open`);
}

export function Mermaid({
  code,
  className,
  children,
}: {
  code: string;
  className?: string;
  /** What to show until the diagram renders — the fence as a code block,
   *  which is both the honest thing during streaming and the whole error
   *  story afterwards. */
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
        // `initialize` sets one global config, so it is re-applied per render
        // rather than once: the theme may have flipped since the last diagram.
        // It is inside the `try` because it is not only a setter — it derives
        // the whole palette there and then, and a colour it cannot read throws
        // from here rather than from `render`.
        mermaid.initialize(config(dark));
        // Asked separately from `render` so a half-written fence is a `false`
        // rather than a throw — and, with `suppressErrors`, without mermaid
        // logging a parse error to the console for every delta that arrives.
        if (!(await mermaid.parse(code, { suppressErrors: true })) || !live) return;
        const out = await mermaid.render(id, code);
        if (!live) return;
        setSize(naturalSize(out.svg));
        setSvg(out.svg);
      } catch {
        // Nothing here may reject into the app: this runs from a timer, so an
        // escaping error is an unhandled rejection rather than something a
        // boundary could catch. Keep whatever is on screen instead — during
        // streaming that is the last frame that worked, and on a first render
        // it is the source, which is what a fence looked like before this
        // component existed.
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
      // What this picture was written as. A selection over an SVG copies its
      // labels in document order, which is not a diagram — `selectionMarkdown`
      // reads the fence back off here instead.
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
                // **The only way into the lightbox.** Revealed on hover like
                // every other secondary control in this app — a message's
                // actions, a tab's close button — but unlike those it is not a
                // shortcut for something the surface also does: pressing the
                // picture pans it (`Scroller`) and nothing else opens it, so
                // this is the whole door.
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

/** The picture as it sits in the reply: fitted to the column, and scrolling
 *  past the box `.diagram` holds it in (`index.css`) — sideways past the width
 *  floor, downwards past the height cap.
 *
 *  **A press pans; it does not open the diagram.** Opening is the expand
 *  control's job and only its job, because a figure is something you read past
 *  and a click that swallowed the page was a trap — every attempt to select a
 *  label, or to nudge a wide diagram sideways, ended in a full-window
 *  lightbox. So there is no click gesture here at all, and the cursor says
 *  "grab" only when there is somewhere to grab it to.
 */
function Scroller({ svg, natural }: { svg: string; natural: DiagramSize | null }) {
  /**
   * The two widths `.diagram` clamps between — the whole sizing policy, in two
   * numbers, because only this file knows both what mermaid drew and the size
   * it drew the labels at.
   *
   * One sentence: *a diagram is drawn as large as it can be without passing
   * the column, the size it was drawn at, or `TARGET_HEIGHT_PX` — and never so
   * small that a label drops under `MIN_LABEL_PX`.*
   *
   *   - `--diagram-min` is that last clause: the width at which a label is
   *     exactly `MIN_LABEL_PX`. Past it the diagram overflows instead of
   *     shrinking further.
   *   - `--diagram-max` is the other three. It starts at the natural width
   *     (labels at their drawn size) and comes down for a *tall* diagram until
   *     it either meets the height target or hits the floor — which is how a
   *     tall one ends up at 11px and a short one at 13px without either being
   *     a special case. The column is the third clause and CSS handles it, as
   *     `100%` between the two.
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
  /** Whether the box actually clips the picture. A grab cursor on a diagram
   *  that fits is a promise the drag cannot keep, and with the height cap in
   *  `index.css` the overflow is now as often vertical as horizontal. */
  const [pannable, setPannable] = useState(false);

  /**
   * Tell `.diagram` which of its four edges have something behind them, so the
   * mask there can fade that edge — the picture running out under a soft edge
   * rather than being guillotined by the box.
   *
   * Written to the element rather than held in state: this runs on every scroll
   * event, and all four values are usually unchanged, so a render per frame
   * would be work thrown away before it was seen. The same pass measures
   * `pannable`, which needs the identical six numbers.
   *
   * The `> 1` is not sloppiness. A clamped, scaled width leaves sub-pixel slack
   * between `scrollWidth` and `clientWidth` on a picture that fits exactly, and
   * at `> 0` that slack reads as overflow — a permanent fade on the edge of a
   * diagram with nothing behind it, and a grab cursor that cannot pan.
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
    // The column this sits in resizes — the side panel opens, the window
    // changes — and both the overflow and which edges are faded change with it.
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
      // Captured only once the gesture is known to be a drag rather than a
      // press, so a press that goes nowhere stays the page's to handle.
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
      // Sizing is the `.diagram` rule in `index.css`; the two widths it
      // clamps between are handed over here — see `bounds` above.
      className={cn("diagram", pannable && "cursor-grab active:cursor-grabbing")}
      style={bounds}
      role="img"
      onScroll={sync}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      // `bindFunctions` is deliberately not called: under `strict` it can only
      // attach tooltips, and this is a picture in a reply, not a control.
      dangerouslySetInnerHTML={{ __html: svg }}
    />
  );
}
