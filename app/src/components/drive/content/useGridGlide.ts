import { useEffect, type RefObject } from "react";

/** A width change this big in one frame is the details panel (256px) opening
 *  or closing, not a window being dragged wider a few pixels at a time. */
const MIN_JUMP = 48;
const DURATION = 220;
const EASE = "cubic-bezier(0.2, 0, 0, 1)";
/** Cards (`itemProps`) and the headings between their runs. */
const GLIDES = "[data-drive-item], [data-glide]";

interface Box { x: number; y: number; w: number; h: number }

/** Layout boxes, unaffected by scroll, transforms and page zoom: every glider
 *  is positioned against the scroll box, which is `relative`. */
function measure(root: HTMLElement): Map<Element, Box> {
  const boxes = new Map<Element, Box>();
  for (const el of root.querySelectorAll<HTMLElement>(GLIDES)) {
    boxes.set(el, { x: el.offsetLeft, y: el.offsetTop, w: el.offsetWidth, h: el.offsetHeight });
  }
  return boxes;
}

/**
 * Plays each glider from its old box to its new one, for those near the
 * screen; the rest just move. Gliders only slide, never scale: a scaled
 * heading or card label stretches its text. A card that grew is drawn at its
 * new size from the start, so a clip opening from its old size keeps it off
 * its neighbours; one that shrank simply has wider gaps for a moment.
 */
function glide(root: HTMLElement, before: Map<Element, Box>, after: Map<Element, Box>) {
  const top = root.scrollTop - root.clientHeight / 2;
  const bottom = root.scrollTop + root.clientHeight * 1.5;
  const timing = { duration: DURATION, easing: EASE };
  for (const [el, to] of after) {
    const from = before.get(el);
    if (!from || !to.w || !to.h) continue;
    if (Math.max(from.y, to.y) > bottom || Math.min(from.y + from.h, to.y + to.h) < top) continue;
    const dx = from.x - to.x;
    const dy = from.y - to.y;
    const cutX = Math.max(0, to.w - from.w);
    const cutY = Math.max(0, to.h - from.h);
    if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5 && cutX < 0.5 && cutY < 0.5) continue;
    for (const running of el.getAnimations()) running.cancel();
    el.animate([{ transform: `translate(${dx}px, ${dy}px)` }, { transform: "none" }], timing);
    if ((cutX >= 0.5 || cutY >= 0.5) && el.hasAttribute("data-drive-item")) {
      const round = getComputedStyle(el).borderTopLeftRadius;
      el.animate(
        [{ clipPath: `inset(0 ${cutX}px ${cutY}px 0 round ${round})` }, { clipPath: `inset(0 0 0 0 round ${round})` }],
        timing,
      );
    }
  }
}

/**
 * When the grid's width jumps — the details panel opening or closing — its
 * cards and headings glide from where they were to where the new column
 * count puts them, instead of every card snapping in one frame. The boxes
 * are re-measured a frame after the cards change, so the jump has the
 * layout from before it; the resize observer runs after layout and before
 * paint, so the first painted frame is already the old position.
 */
export function useGridGlide(ref: RefObject<HTMLElement | null>) {
  useEffect(() => {
    const root = ref.current;
    if (!root) return;
    const still = window.matchMedia("(prefers-reduced-motion: reduce)");
    let width = root.clientWidth;
    let boxes = measure(root);
    let frame = 0;

    const resize = new ResizeObserver(() => {
      const next = root.clientWidth;
      const jumped = Math.abs(next - width) >= MIN_JUMP;
      width = next;
      const now = measure(root);
      if (jumped && !still.matches) glide(root, boxes, now);
      boxes = now;
    });
    const changed = new MutationObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        if (root.clientWidth === width) boxes = measure(root);
      });
    });
    resize.observe(root);
    changed.observe(root, { childList: true, subtree: true });
    return () => {
      resize.disconnect();
      changed.disconnect();
      cancelAnimationFrame(frame);
    };
  }, [ref]);
}
