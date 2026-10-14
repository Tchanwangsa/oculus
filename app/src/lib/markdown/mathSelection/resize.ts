import type { Formula } from "./formula";

/** Inline spans do not report resize changes. Watch the containing block
 *  for wrapping and KaTeX's inline blocks for fonts changing the glyphs. */
export function watchLayout(formula: Formula, redraw: () => void): () => void {
  const observer = new ResizeObserver(redraw);
  for (const base of formula.html.querySelectorAll(":scope > .base")) observer.observe(base);
  let block: HTMLElement | null = formula.root.parentElement;
  while (block && ["inline", "contents"].includes(getComputedStyle(block).display)) block = block.parentElement;
  if (block) observer.observe(block);
  document.fonts.addEventListener("loadingdone", redraw);
  return () => {
    observer.disconnect();
    document.fonts.removeEventListener("loadingdone", redraw);
  };
}
