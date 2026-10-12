import type { Box } from "./box";

/** Px a band reaches past its ink above and below: a glyph's paint (a
 *  script, `\sum`) overshoots its font's line box a little. */
const BAND_PAD = 2;

/** Pad without overlapping any band that shares horizontal space. Array
 *  delimiters can sit between two rows in the sorted list, so neighbours
 *  alone do not catch every overlap. */
export function paddedBands(boxes: readonly Box[], pad = BAND_PAD): Box[] {
  const out = boxes.map((b) => ({ ...b, top: b.top - pad, bottom: b.bottom + pad }));
  for (let i = 1; i < boxes.length; i++) {
    for (let j = 0; j < i; j++) {
      const [above, below] = [boxes[j], boxes[i]];
      if (below.left >= above.right || below.right <= above.left || out[i].top >= out[j].bottom) continue;
      const cut = Math.max(above.bottom, Math.min(below.top, (above.bottom + below.top) / 2));
      out[j].bottom = Math.min(out[j].bottom, cut);
      out[i].top = Math.max(out[i].top, cut);
    }
  }
  return out;
}

/** Draws `boxes` (`bands`, in `layer`'s frame, top to bottom) as
 *  absolutely positioned children of `layer`, reusing its elements.
 *  Padding is clipped between rows so their tint never doubles. */
export function paintBands(layer: HTMLElement, boxes: readonly Box[], className: string, pad = BAND_PAD) {
  while (layer.children.length > boxes.length) layer.lastElementChild!.remove();
  while (layer.children.length < boxes.length) {
    const band = document.createElement("span");
    band.className = className;
    layer.append(band);
  }
  paddedBands(boxes, pad).forEach((b, i) => {
    const band = layer.children[i] as HTMLElement;
    band.style.left = `${b.left}px`;
    band.style.top = `${b.top}px`;
    band.style.width = `${b.right - b.left}px`;
    band.style.height = `${b.bottom - b.top}px`;
  });
}
