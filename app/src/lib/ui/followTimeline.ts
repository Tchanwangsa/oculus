import { anchorDrift, HELD_ANCHOR, pickAnchor, SKIPPED_CONTENT, type Anchor } from "./scrollAnchor";

/** One live collection of timeline rows, without a selector walk per scroll. */
export const ANCHOR_ROW = "scroll-anchor-row";
// scrollHeight/clientHeight are integers, while scrollTop can be fractional.
const BOTTOM_EPSILON = 1;
// The arrow has its own display threshold; this never delays releasing follow.
const SHOW_ARROW_PX = 16;

/** Own the scroll listeners and resize corrections independently of React. */
export function followTimeline(
  el: HTMLElement,
  content: HTMLElement,
  running: () => boolean,
  onAway: (away: boolean) => void,
) {
  const rows = content.getElementsByClassName(ANCHOR_ROW);
  let pinned = true;
  let opening = true;
  const bottom = () => Math.max(0, el.scrollHeight - el.clientHeight);
  const position = () => Math.max(0, Math.min(bottom(), el.scrollTop));
  let at = position();
  let anchor: Anchor | null = null;
  let held: Element | null = null;
  let touchY: number | undefined;
  let away = false;
  const atBottom = () => bottom() - position() <= BOTTOM_EPSILON;
  const publish = () => {
    const next = bottom() - position() > (away ? BOTTOM_EPSILON : SHOW_ARROW_PX);
    if (next !== away) onAway(away = next);
  };
  const remember = () => {
    anchor = pickAnchor(el, rows);
    const row = anchor?.node.closest(`.${ANCHOR_ROW}`) ?? null;
    if (held === row) return;
    held?.removeAttribute(HELD_ANCHOR);
    held = row;
    held?.setAttribute(HELD_ANCHOR, "");
  };
  const release = () => {
    opening = pinned = false;
    at = position();
    remember();
  };
  const moved = () => {
    const top = position();
    if (top === at) return false;
    // Even a fractional upward move releases; only returning down to the
    // actual bottom can re-arm following. Our writes update `at` immediately.
    pinned = top > at && atBottom();
    opening = false;
    at = top;
    remember();
    publish();
    return true;
  };
  const scrollToBottom = () => {
    opening = false;
    pinned = true;
    // Instant writes have no animation that can keep fighting user input.
    el.scrollTop = bottom();
    at = position();
    remember();
    publish();
  };
  const onScroll = () => { moved(); publish(); };
  const onWheel = (event: WheelEvent) => {
    if (event.deltaY < 0) release();
    else if (event.deltaY > 0 && atBottom()) pinned = true;
  };
  const onTouchStart = (event: TouchEvent) => { touchY = event.touches[0]?.clientY; };
  const onTouchMove = (event: TouchEvent) => {
    const y = event.touches[0]?.clientY;
    if (y != null && touchY != null && y > touchY) release();
    touchY = y;
  };
  const onKey = (event: KeyboardEvent) => {
    if ((event.target as Element | null)?.closest("input, textarea, [contenteditable=true]")) return;
    if (event.key === "ArrowUp" || event.key === "PageUp" || event.key === "Home" || (event.key === " " && event.shiftKey)) release();
  };
  const ro = new ResizeObserver(() => {
    if (!el.clientHeight) return;
    // WebKit exposes rubber-band overscroll in scrollTop and rects. Its
    // rebound is native motion, so don't correct it or flash the arrow.
    if (el.scrollTop < 0 || el.scrollTop > bottom()) {
      publish();
      return;
    }
    // Native scrolling can precede its event. Never interpret that movement
    // as layout drift and undo it from the resize callback.
    const scrolled = moved();
    if (opening) {
      if (rows.length || (running() && content.offsetHeight)) scrollToBottom();
      return;
    }
    if (pinned && running()) {
      scrollToBottom();
      return;
    }
    if (!scrolled) {
      const drift = anchor && anchorDrift(el, anchor, content);
      if (drift == null) remember();
      else if (Math.abs(drift) >= 0.5) {
        el.scrollTop += drift;
        at = position();
      }
      if (anchor?.deferred && !anchor.node.hasAttribute(SKIPPED_CONTENT)) remember();
    }
    if (!atBottom()) pinned = false;
    publish();
  });
  el.addEventListener("scroll", onScroll, { passive: true });
  el.addEventListener("wheel", onWheel, { passive: true });
  el.addEventListener("touchstart", onTouchStart, { passive: true });
  el.addEventListener("touchmove", onTouchMove, { passive: true });
  el.addEventListener("keydown", onKey);
  ro.observe(content);
  ro.observe(el);
  return {
    scrollToBottom,
    destroy() {
      ro.disconnect();
      el.removeEventListener("scroll", onScroll);
      el.removeEventListener("wheel", onWheel);
      el.removeEventListener("touchstart", onTouchStart);
      el.removeEventListener("touchmove", onTouchMove);
      el.removeEventListener("keydown", onKey);
      held?.removeAttribute(HELD_ANCHOR);
    },
  };
}
