import type { Box } from "./box";

/** Px a band reaches past its ink above and below: a glyph's paint (a
 *  script, `\sum`) overshoots its font's line box a little. */
const BAND_PAD = 2;

/** Draws `boxes` (`bands`, in `layer`'s frame, top to bottom) as
 *  absolutely positioned children of `layer` with class `className`,
 *  reusing its elements. Each is padded above and below, except where that
 *  would overlap the band next to it, which would paint darker. */
export function paintBands(layer: HTMLElement, boxes: readonly Box[], className: string) {
  while (layer.children.length > boxes.length) layer.lastElementChild!.remove();
  while (layer.children.length < boxes.length) {
    const band = document.createElement("span");
    band.className = className;
    layer.append(band);
  }
  const tops = boxes.map((b) => b.top - BAND_PAD);
  const bottoms = boxes.map((b) => b.bottom + BAND_PAD);
  for (let i = 1; i < boxes.length; i++) {
    const [above, below] = [boxes[i - 1], boxes[i]];
    const sideBySide = below.left >= above.right || below.right <= above.left;
    if (!sideBySide && tops[i] < bottoms[i - 1]) {
      const cut = Math.max(above.bottom, Math.min(below.top, (above.bottom + below.top) / 2));
      bottoms[i - 1] = cut;
      tops[i] = cut;
    }
  }
  boxes.forEach((b, i) => {
    const band = layer.children[i] as HTMLElement;
    band.style.left = `${b.left}px`;
    band.style.top = `${tops[i]}px`;
    band.style.width = `${b.right - b.left}px`;
    band.style.height = `${bottoms[i] - tops[i]}px`;
  });
}
