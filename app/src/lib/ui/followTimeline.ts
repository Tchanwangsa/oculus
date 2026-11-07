import { anchorDrift, HELD_ANCHOR, pickAnchor, SKIPPED_CONTENT, zoomScale, type Anchor } from "./scrollAnchor";

/** One live collection of timeline rows, without a selector walk per scroll. */
export const ANCHOR_ROW = "scroll-anchor-row";
// scrollHeight/clientHeight are integers, while scrollTop can be fractional.
const BOTTOM_EPSILON = 1;
// The arrow has its own display threshold; this never delays releasing follow.
const SHOW_ARROW_PX = 16;
// A reader scrolling back down re-arms following this close to the bottom:
// during a reply the bottom moves every fold, so landing exactly on it is luck.
const REARM_PX = 32;
// How long after the reader's last input a scroll still counts as theirs.
// Momentum keeps sending wheel events, so this only has to bridge gaps.
const INPUT_MS = 250;

const WINDOW_INPUT = ["pointerdown", "pointerup", "pointercancel", "wheel", "keydown", "touchstart", "touchmove"] as const;

let declared = -Infinity;
/** For code that scrolls a timeline on the reader's behalf from somewhere a
 *  keystroke doesn't count (find typed into its field): what moves next is theirs. */
export function noteScrollIntent() {
  declared = performance.now();
}

/** Typing in a field scrolls nothing here, though it resizes the composer. */
function typing(event: Event): boolean {
  if (event.type !== "keydown") return false;
  return !!(event.target as Element | null)?.closest?.("input, textarea, [contenteditable]:not([contenteditable=false])");
}

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
  // The scroll range when `at` was taken: an upward move that leaves the
  // reader at a smaller range's bottom is the browser clamping, not input.
  let range = bottom();
  // The scroller's own size: when it changes, a reader at the bottom stays there.
  let size = [el.clientWidth, el.clientHeight];
  // The browser also clamps during layouts between frames (content shrinking
  // and regrowing within one task), so an upward move with no recent input
  // anywhere in the window is layout, not the reader.
  let lastInput = -Infinity;
  let pressed = false;
  const reading = () => pressed || performance.now() - Math.max(lastInput, declared) < INPUT_MS;
  // Following the bottom through layout that lands after the jump which
  // started it (opening, the arrow, a reply and its re-render as a committed
  // row). Ends at the reader's next input of their own.
  let trailing = false;
  let anchor: Anchor | null = null;
  let pressedRow: Anchor | null = null;
  let held: Element | null = null;
  let touchY: number | undefined;
  let away = false;
  const atBottom = () => bottom() - position() <= BOTTOM_EPSILON;
  const nearBottom = () => bottom() - position() <= REARM_PX;
  const publish = () => {
    const next = bottom() - position() > (away ? BOTTOM_EPSILON : SHOW_ARROW_PX);
    if (next !== away) onAway(away = next);
  };
  const remember = () => {
    pressedRow = null;
    anchor = pickAnchor(el, rows);
    const row = anchor?.node.closest(`.${ANCHOR_ROW}`) ?? null;
    if (held === row) return;
    held?.removeAttribute(HELD_ANCHOR);
    held = row;
    held?.setAttribute(HELD_ANCHOR, "");
  };
  const mark = () => {
    at = position();
    range = bottom();
  };
  const release = () => {
    // Layout that landed since the last frame is corrected first, or
    // re-picking the anchor would bake it in.
    correct();
    opening = pinned = trailing = false;
    mark();
    remember();
  };
  const moved = () => {
    const top = position();
    if (top === at) return false;
    // Even a fractional upward move by the reader releases; only returning
    // down to the actual bottom can re-arm following. Our writes update `at`
    // immediately.
    const clamped = top < at && ((bottom() < range && atBottom()) || !reading());
    pinned = top > at ? nearBottom() : clamped && pinned;
    // A contenteditable deletion can clamp and restore the viewport within
    // one edit, leaving no resize to restore an idle timeline's bottom.
    if (clamped && pinned) trailing = true;
    opening = false;
    mark();
    remember();
    return true;
  };
  const stick = () => {
    opening = false;
    pinned = true;
    // Instant writes have no animation that can keep fighting user input.
    if (el.scrollTop !== bottom()) el.scrollTop = bottom();
    mark();
    remember();
    publish();
  };
  const scrollToBottom = () => {
    trailing = true;
    stick();
  };
  /** Undo layout drift above the reader: how far the anchor moved, less the
   *  scroller's own movement since it was taken, so it holds even mid-scroll.
   *  `at` moves with the write, leaving the native movement for `moved`. */
  const correct = () => {
    if (opening || (pinned && trailing)) return;
    // A pressed control that re-rendered away falls back to its row.
    if (pressedRow && anchor && !anchor.node.isConnected) anchor = pressedRow;
    const drift = anchor && anchorDrift(el, anchor, content);
    if (drift == null) {
      remember();
      return;
    }
    const shift = drift + (position() - at);
    if (Math.abs(shift) < 0.5) return;
    const before = position();
    el.scrollTop = before + shift;
    at += position() - before;
  };
  const onScroll = () => {
    if (el.scrollTop >= 0 && el.scrollTop <= bottom()) correct();
    moved();
    // Re-armed short of the bottom during a reply: close the last few pixels.
    if (pinned && (trailing || running()) && !atBottom()) stick();
    publish();
  };
  const onWindowInput = (event: Event) => {
    if (typing(event)) {
      // The click that focused the composer must not turn its next native
      // editing clamp into a reader scroll, even within the input window.
      lastInput = -Infinity;
      return;
    }
    lastInput = performance.now();
    if (event.type === "pointerdown") pressed = true;
    else if (event.type === "pointerup" || event.type === "pointercancel") pressed = false;
  };
  // Any input of the reader's own (say, opening a folded row) ends trailing,
  // and a press holds what was pressed still: whatever it opens or answers
  // grows around it rather than shoving it away from the pointer.
  const onInput = (event: Event) => {
    trailing = false;
    const target = event.target as Element | null;
    if (typeof target?.closest !== "function") return;
    const row = target.closest(`.${ANCHOR_ROW}`);
    if (!row || !content.contains(row)) return;
    const box = el.getBoundingClientRect();
    const scale = zoomScale(box.height, el.offsetHeight);
    const top = (node: Element) => (node.getBoundingClientRect().top - box.top) / scale;
    anchor = { node: target, offset: top(target) };
    pressedRow = { node: row, offset: top(row) };
    at = position();
  };
  const onWheel = (event: WheelEvent) => {
    trailing = false;
    if (event.deltaY < 0) release();
    else if (event.deltaY > 0 && nearBottom()) pinned = true;
  };
  const onTouchStart = (event: TouchEvent) => { trailing = false; touchY = event.touches[0]?.clientY; };
  const onTouchMove = (event: TouchEvent) => {
    const y = event.touches[0]?.clientY;
    if (y != null && touchY != null && y > touchY) release();
    touchY = y;
  };
  const onKey = (event: KeyboardEvent) => {
    trailing = false;
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
    // Native scrolling can precede its event; `correct` leaves it alone.
    correct();
    moved();
    if (opening) {
      if (rows.length || (running() && content.offsetHeight)) scrollToBottom();
      return;
    }
    const resized = el.clientWidth !== size[0] || el.clientHeight !== size[1];
    size = [el.clientWidth, el.clientHeight];
    if (pinned && running()) trailing = true;
    if (pinned && (trailing || resized)) {
      stick();
      return;
    }
    if (anchor?.deferred && !anchor.node.hasAttribute(SKIPPED_CONTENT)) remember();
    if (!atBottom()) pinned = false;
    publish();
  });
  el.addEventListener("scroll", onScroll, { passive: true });
  el.addEventListener("wheel", onWheel, { passive: true });
  el.addEventListener("touchstart", onTouchStart, { passive: true });
  el.addEventListener("touchmove", onTouchMove, { passive: true });
  el.addEventListener("keydown", onKey);
  el.addEventListener("pointerdown", onInput);
  const win: EventTarget = el.ownerDocument?.defaultView ?? globalThis;
  for (const type of WINDOW_INPUT) win.addEventListener(type, onWindowInput, { capture: true, passive: true });
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
      el.removeEventListener("pointerdown", onInput);
      for (const type of WINDOW_INPUT) win.removeEventListener(type, onWindowInput, { capture: true });
      held?.removeAttribute(HELD_ANCHOR);
    },
  };
}
