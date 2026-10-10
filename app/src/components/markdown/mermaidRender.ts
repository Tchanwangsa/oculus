import type { DiagramSize } from "@/components/markdown/DiagramLightbox";
import { isDark } from "@/lib/ui/theme";
import { load } from "@/components/markdown/mermaid/load";
import {
  LABEL_PX,
  MIN_LABEL_PX,
  TARGET_HEIGHT_PX,
  config,
} from "@/components/markdown/mermaid/theme";

/**
 * Mermaid source to themed SVG, shared by the markdown `Mermaid` component and
 * the note editor's Live mode (`editor/live-preview/widgets/media.ts`).
 */

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
