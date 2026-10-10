import type { PdfLine } from "@/lib/pdf/pdfView";

/**
 * A page's selectable text: one transparent, absolutely placed `<span>` per
 * line over the canvas, a `<br>` between lines, so the layer's text nodes read
 * the page in order (`lib/pdf/pdfSelectionMarkdown` and citations rely on this).
 * Geometry is in points times `--pdf-unit` (CSS px per point, set on the
 * page), so a zoom restyles the layer without rebuilding it or dropping a
 * selection. A line is stretched to its box with `scaleX`, from a canvas
 * measurement — never a DOM one, which page zoom skews (docs/ui.md).
 */

/** The spans' font, measured by the same name. */
const FONT = "sans-serif";
const PROBE = 100;

let probe: CanvasRenderingContext2D | null = null;

function measure(text: string): number {
  if (!probe) {
    probe = document.createElement("canvas").getContext("2d");
    if (!probe) return 0;
    probe.font = `${PROBE}px ${FONT}`;
  }
  return probe.measureText(text).width;
}

const pt = (v: number) => `calc(var(--pdf-unit) * ${v}px)`;

/** The layer's last child: an empty block that, while a drag selects, sits
 *  under the spans so a pointer in a gap between lines lands next to the line
 *  the selection ends on, not at the page's end (`watchSelection`). */
const END = "pdf-text-end";

export function buildTextLayer(layer: HTMLElement, lines: readonly PdfLine[]): void {
  const frag = document.createDocumentFragment();
  let first = true;
  for (const line of lines) {
    if (!line.text) continue;
    if (!first) frag.append(document.createElement("br"));
    first = false;
    const size = line.vertical ? line.width : line.height;
    const length = line.vertical ? line.height : line.width;
    const natural = (measure(line.text) * size) / PROBE;
    const stretch = natural > 0 && length > 0 ? length / natural : 1;
    const span = document.createElement("span");
    span.textContent = line.text;
    const s = span.style;
    s.left = pt(line.vertical ? line.x + line.width : line.x);
    s.top = pt(line.y);
    s.fontSize = pt(size);
    s.transform = `${line.vertical ? "rotate(90deg) " : ""}scaleX(${stretch})`;
    frag.append(span);
  }
  const end = document.createElement("div");
  end.className = END;
  frag.append(end);
  layer.replaceChildren(frag);
}

/**
 * While a drag selects inside `root`'s text layers, moves each layer's end
 * block to just after the span the selection's moving edge is in, and marks
 * the layer `selecting` so the block covers it (`index.css`). Without it a
 * pointer between two lines hits the layer itself and the selection jumps to
 * the page's start or end.
 */
export function watchSelection(root: HTMLElement): () => void {
  let down = false;
  let previous: Range | null = null;
  const doc = root.ownerDocument;

  const reset = () => {
    for (const layer of root.querySelectorAll<HTMLElement>(".textLayer.selecting")) {
      layer.classList.remove("selecting");
      const end = layer.querySelector(`:scope > .${END}`);
      if (end && end !== layer.lastChild) layer.append(end);
    }
    previous = null;
  };

  const onDown = (e: PointerEvent) => {
    down = root.contains(e.target as Node);
  };
  const onUp = () => {
    down = false;
    reset();
  };

  const onChange = () => {
    if (!down) return;
    const selection = doc.getSelection();
    if (!selection?.rangeCount) return reset();
    const range = selection.getRangeAt(0);
    // The edge that moved since the last change; the end by default.
    const atStart =
      !!previous &&
      (range.compareBoundaryPoints(Range.END_TO_END, previous) === 0 ||
        range.compareBoundaryPoints(Range.START_TO_END, previous) === 0);
    previous = range.cloneRange();
    let node: Node | null = atStart ? range.startContainer : range.endContainer;
    if (node.nodeType === Node.TEXT_NODE) node = node.parentNode;
    const span = node instanceof HTMLElement ? node.closest(".textLayer > span") : null;
    const layer = span?.parentElement;
    if (!span || !layer || !root.contains(layer)) return;
    const end = layer.querySelector<HTMLElement>(`:scope > .${END}`);
    if (!end) return;
    layer.classList.add("selecting");
    const before = atStart ? span : span.nextSibling;
    if (end !== before && end.nextSibling !== before) layer.insertBefore(end, before);
  };

  doc.addEventListener("pointerdown", onDown, true);
  doc.addEventListener("pointerup", onUp, true);
  window.addEventListener("blur", onUp);
  doc.addEventListener("selectionchange", onChange);
  return () => {
    doc.removeEventListener("pointerdown", onDown, true);
    doc.removeEventListener("pointerup", onUp, true);
    window.removeEventListener("blur", onUp);
    doc.removeEventListener("selectionchange", onChange);
  };
}
