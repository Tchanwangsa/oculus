export type ScrollFadeAxis = "x" | "y" | "xy";

/** `> 1`, not `> 0`: a scaled box leaves sub-pixel slack that would read as overflow. */
const SLACK = 1;

/**
 * Write the per-edge fade vars (0/1) that `[data-scroll-fade]` in `index.css`
 * turns into a mask. On the element, not in state: this runs per scroll event.
 * Shared by `useScrollFade` and the CodeMirror maths palette, which has no React.
 */
export function syncScrollFade(el: HTMLElement, axis: ScrollFadeAxis) {
  // Read all geometry before changing mask styles: interleaving the two can
  // force layout again for every scroll tick.
  const x = axis !== "y" ? {
    start: el.scrollLeft > SLACK,
    end: el.scrollLeft < el.scrollWidth - el.clientWidth - SLACK,
  } : null;
  const y = axis !== "x" ? {
    start: el.scrollTop > SLACK,
    end: el.scrollTop < el.scrollHeight - el.clientHeight - SLACK,
  } : null;
  const bar = axis === "y" ? `${el.offsetWidth - el.clientWidth}px` : null;

  if (el.dataset.scrollFade !== axis) el.dataset.scrollFade = axis;
  const set = (name: string, value: string) => {
    if (el.style.getPropertyValue(name) !== value) el.style.setProperty(name, value);
  };
  if (x) {
    set("--fade-x-start", x.start ? "1" : "0");
    set("--fade-x-end", x.end ? "1" : "0");
  }
  if (y) {
    set("--fade-y-start", y.start ? "1" : "0");
    set("--fade-y-end", y.end ? "1" : "0");
  }
  // A mask would fade a classic scrollbar with the content; the mask leaves
  // this much of the right edge solid. Single-axis only: two bars can't both
  // be spared by one strip.
  if (bar !== null) set("--fade-bar", bar);
}
