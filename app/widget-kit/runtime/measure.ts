import { post } from "./post";

/**
 * Reports the root's height whenever it changes, and once when its content
 * is wider than the frame. The root is `display: flow-root` (kit.css), so
 * its box holds its children's margins; `html` and `body` never scroll, so
 * the frame must be exactly this tall.
 */
export function measure(root: HTMLElement) {
  let height = -1;
  let overflowing = false;
  const report = () => {
    const h = Math.ceil(root.getBoundingClientRect().height);
    if (h !== height) {
      height = h;
      post({ kind: "height", height: h });
    }
    const frameWidth = document.documentElement.clientWidth;
    const wide = root.scrollWidth > frameWidth + 1;
    if (wide && !overflowing) post({ kind: "overflow", width: root.scrollWidth, frameWidth });
    overflowing = wide;
  };
  new ResizeObserver(report).observe(root);
  report();
}
