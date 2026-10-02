import type { MermaidConfig } from "mermaid";
import type { DiagramSize } from "@/components/markdown/DiagramLightbox";
import { isDark } from "@/lib/theme";

/**
 * Mermaid source to themed SVG, shared by the markdown `Mermaid` component and
 * the note editor's Live mode (`editor/widgets.ts`).
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
    // On failure the caller shows the source instead of mermaid's red card.
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

// ── Rendering ────────────────────────────────────────────────────────────────

/** Per-diagram ids: mermaid scopes the SVG's `<style>` and `<marker>`s by it. */
let seq = 0;

export function mermaidId(): string {
  return `oculus-mermaid-${++seq}`;
}

/** The drawn size, read from the SVG's `viewBox`; falls back to its inline
 *  `max-width` (square guess) for types that emit no viewBox. */
function naturalSize(svg: string): DiagramSize | null {
  const box = /viewBox="\s*[\d.-]+\s+[\d.-]+\s+([\d.]+)\s+([\d.]+)/.exec(svg);
  if (box) return { width: Number(box[1]), height: Number(box[2]) };
  const capped = /max-width:\s*([\d.]+)px/.exec(svg);
  return capped ? { width: Number(capped[1]), height: Number(capped[1]) } : null;
}

export interface Diagram {
  svg: string;
  size: DiagramSize | null;
}

/**
 * `code` drawn in the current theme under `id`, or null when it does not parse
 * (a half-written fence parses to `false` quietly). Throws only on a mermaid
 * failure past parsing.
 */
export async function renderMermaid(code: string, id: string): Promise<Diagram | null> {
  const mermaid = await load();
  // Global config, re-applied per render (the theme may have flipped). A bad
  // colour throws from here.
  mermaid.initialize(config(isDark()));
  if (!(await mermaid.parse(code, { suppressErrors: true }))) return null;
  const out = await mermaid.render(id, code);
  return { svg: out.svg, size: naturalSize(out.svg) };
}

/**
 * The widths `.diagram` clamps `100%` between: as large as possible without
 * passing the natural size or `TARGET_HEIGHT_PX`, never so small a label
 * drops under `MIN_LABEL_PX` (`--diagram-min`; past it, overflow).
 */
export function diagramBounds(natural: DiagramSize): { "--diagram-max": string; "--diagram-min": string } {
  const floor = MIN_LABEL_PX / LABEL_PX;
  const ceiling = Math.min(1, Math.max(TARGET_HEIGHT_PX / natural.height, floor));
  return {
    "--diagram-max": `${natural.width * ceiling}px`,
    "--diagram-min": `${natural.width * floor}px`,
  };
}
