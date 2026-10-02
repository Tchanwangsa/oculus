export type ScrollFadeAxis = "x" | "y" | "xy";

/** `> 1`, not `> 0`: a scaled box leaves sub-pixel slack that would read as overflow. */
const SLACK = 1;

/**
 * Write the per-edge fade vars (0/1) that `[data-scroll-fade]` in `index.css`
 * turns into a mask. On the element, not in state: this runs per scroll event.
 * Shared by `useScrollFade` and the CodeMirror maths palette, which has no React.
 */
export function syncScrollFade(el: HTMLElement, axis: ScrollFadeAxis) {
  el.dataset.scrollFade = axis;
  const set = (name: string, on: boolean) => el.style.setProperty(name, on ? "1" : "0");
  if (axis !== "y") {
    const over = el.scrollWidth - el.clientWidth;
    set("--fade-x-start", el.scrollLeft > SLACK);
    set("--fade-x-end", el.scrollLeft < over - SLACK);
  }
  if (axis !== "x") {
    const over = el.scrollHeight - el.clientHeight;
    set("--fade-y-start", el.scrollTop > SLACK);
    set("--fade-y-end", el.scrollTop < over - SLACK);
  }
  // A mask would fade a classic scrollbar with the content; the mask leaves
  // this much of the right edge solid. Single-axis only: two bars can't both
  // be spared by one strip.
  if (axis === "y") el.style.setProperty("--fade-bar", `${el.offsetWidth - el.clientWidth}px`);
}
