import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { isVertical, usePlayerPrefs, type Dock, type DockTab } from "@/stores/playerPrefsStore";

export { isVertical, type Dock };

const MIN_H = 88;
const MIN_W = 220;
/**
 * …and a floor under that floor while Chat is the tab in front.
 *
 * A chapter row and a transcript cue are a line of text and read fine in
 * 220px; an agent's reply is markdown with lists, tables and fenced code in
 * it, and a composer with a model picker and a send button under that. At the
 * transcript's minimum the code blocks were a horizontal scroller two words
 * wide and the control row wrapped. The floor is raised rather than the rows
 * restyled, because the rows are the Chat page's and there is one timeline.
 *
 */
const CHAT_MIN_H = 260;
const CHAT_MIN_W = 300;

const minSize = (tab: DockTab, vertical: boolean) =>
  vertical ? (tab === "chat" ? CHAT_MIN_H : MIN_H) : tab === "chat" ? CHAT_MIN_W : MIN_W;

/**
 * Room the video stack always keeps for itself.
 *
 * It is also what decides whether a *side* dock fits at all: a container
 * narrower than the panel's own floor plus this cannot hold both, and the two
 * of them together are what a side panel does not have.
 */
const KEEP_H = 140;
const KEEP_W = 300;
/** Pointer travel before a header press counts as a dock drag, not a click. */
const DRAG_SLOP = 5;

/**
 * The edge nearest the pointer — the container split into four triangles, or
 * into two bands where there is no room beside the video, so that the edge
 * previewed under the pointer is the edge the drop can actually deliver.
 */
function nearestEdge(rect: DOMRect, x: number, y: number, sides: boolean): Dock {
  const fx = Math.min(1, Math.max(0, (x - rect.left) / rect.width));
  const fy = Math.min(1, Math.max(0, (y - rect.top) / rect.height));
  const d: [Dock, number][] = [
    ["top", fy],
    ["bottom", 1 - fy],
  ];
  if (sides) d.push(["left", fx], ["right", 1 - fx]);
  return d.reduce((a, b) => (b[1] < a[1] ? b : a))[0];
}

/**
 * Dock side + size for the lecture transcript panel: drag its header to any
 * edge of the player, drag the divider to resize. Both are global player
 * preferences (`playerPrefsStore`), so they carry across lectures and
 * sessions.
 *
 * Pointer maths stays in client coordinates against one `getBoundingClientRect`
 * of the player — safe here because the app zooms the webview page rather than
 * CSS-zooming a container (see docs/frontend.md).
 */
export function useTranscriptDock(containerRef: RefObject<HTMLDivElement | null>) {
  const dock = usePlayerPrefs((s) => s.dock);
  const dockTab = usePlayerPrefs((s) => s.dockTab);
  const height = usePlayerPrefs((s) => s.height);
  const width = usePlayerPrefs((s) => s.width);
  const setPrefs = usePlayerPrefs((s) => s.set);

  // The drag reads the floor out of a ref rather than taking it as a
  // dependency: the pointer listeners are registered once, and re-binding
  // three of them because a tab changed is churn for a number.
  const tabRef = useRef(dockTab);
  tabRef.current = dockTab;
  /** Whether a left/right dock fits at all; see `fitsBeside` below. */
  const fitsRef = useRef(true);

  /** Edge highlighted under the pointer mid-drag; null when not dragging. */
  const [dropTarget, setDropTarget] = useState<Dock | null>(null);
  // Mirrored in a ref: the drop is committed from a pointer handler, and a
  // state updater is the wrong place for that side effect (StrictMode runs
  // updaters twice).
  const dropTargetRef = useRef<Dock | null>(null);

  /**
   * Mirrored into state because the panel needs it in its className: it slides
   * open and shut on a transition, and a transition during a resize drag means
   * the edge trailing 200ms behind the pointer.
   */
  const [resizing, setResizing] = useState(false);

  const drag = useRef<{ x: number; y: number; moved: boolean } | null>(null);
  const resize = useRef<{ dock: Dock; x: number; y: number; size: number } | null>(
    null,
  );

  const startDockDrag = useCallback((e: React.PointerEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    drag.current = { x: e.clientX, y: e.clientY, moved: false };
  }, []);

  /**
   * The player's own size, watched rather than read once. The dock is sized
   * against the box it sits in, and that box is a docked side panel one
   * moment, a full page the next and a fullscreen overlay after that — all
   * three out of one set of preferences.
   */
  const [area, setArea] = useState({ width: 0, height: 0 });
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      setArea((a) => (a.width === width && a.height === height ? a : { width, height }));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [containerRef]);

  /**
   * The side actually drawn, which is the preference wherever it fits.
   *
   * A left or right dock needs its own floor *and* the room the video keeps,
   * and the side panel is narrower than that sum: the panel took the width it
   * was given in a full-page player and left the video a sliver of black
   * beside it. Too narrow, and the dock draws along the bottom instead, where
   * the floor to clear is a height even a panel has plenty of. Like the size
   * floor below, this rewrites nothing — widening the window puts the dock
   * straight back on the edge it was dropped on.
   */
  const fitsBeside = area.width === 0 || area.width >= minSize(dockTab, false) + KEEP_W;
  const drawnDock: Dock = fitsBeside ? dock : "bottom";
  // Read from a ref by the drag, for the same reason the tab's floor is: the
  // pointer listeners are bound once and must not re-bind on a resize.
  fitsRef.current = fitsBeside;
  const vertical = isVertical(drawnDock);

  /**
   * The size actually drawn: the preference, with the tab's floor under it and
   * the container's own room over it.
   *
   * A width stored from the transcript's own minimum would otherwise leave
   * Chat in a 220px panel until someone dragged it out, and a size dragged out
   * in a fullscreen player would otherwise be honoured in a panel a third of
   * that wide — the drag clamps against the container, so anything that never
   * went through a drag has to be clamped here too. The preference survives
   * both, so leaving the tab or the panel hands the transcript back exactly
   * the panel it had, and a drag starts from what is on screen rather than
   * from the number behind it.
   */
  const drawn = (() => {
    const min = minSize(dockTab, vertical);
    const measured = vertical ? area.height : area.width;
    const cap =
      measured > 0
        ? Math.max(min, measured - (vertical ? KEEP_H : KEEP_W))
        : Number.POSITIVE_INFINITY;
    return Math.min(Math.max(min, vertical ? height : width), cap);
  })();

  const startResize = useCallback(
    (e: React.PointerEvent) => {
      if (e.button !== 0) return;
      e.preventDefault();
      (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
      // The drag resizes the dock as drawn, not as stored: a side dock demoted
      // to the bottom has its divider along the bottom, and dragging it has to
      // be the vertical gesture that matches.
      resize.current = { dock: drawnDock, x: e.clientX, y: e.clientY, size: drawn };
      setResizing(true);
      document.body.style.cursor = vertical ? "row-resize" : "col-resize";
      document.body.style.userSelect = "none";
    },
    [drawnDock, vertical, drawn],
  );

  useEffect(() => {
    // A trackpad emits pointermove faster than the display refreshes, and each
    // one here both forces layout (`getBoundingClientRect`) and sets state.
    // Coalescing to one frame is the difference between a smooth drag and a
    // drag that lays the player out several times per painted frame.
    let frame = 0;
    let pending: { clientX: number; clientY: number } | null = null;

    const apply = (e: { clientX: number; clientY: number }) => {
      const rect = containerRef.current?.getBoundingClientRect();

      if (resize.current && rect) {
        const r = resize.current;
        if (isVertical(r.dock)) {
          const min = minSize(tabRef.current, true);
          const dy = e.clientY - r.y;
          // Bottom dock grows upward, top dock downward.
          const next = r.size + (r.dock === "bottom" ? -dy : dy);
          setPrefs({
            height: Math.min(Math.max(min, next), Math.max(min, rect.height - KEEP_H)),
          });
        } else {
          const min = minSize(tabRef.current, false);
          const dx = e.clientX - r.x;
          const next = r.size + (r.dock === "right" ? -dx : dx);
          setPrefs({
            width: Math.min(Math.max(min, next), Math.max(min, rect.width - KEEP_W)),
          });
        }
        return;
      }

      if (drag.current && rect) {
        if (
          !drag.current.moved &&
          Math.abs(e.clientX - drag.current.x) < DRAG_SLOP &&
          Math.abs(e.clientY - drag.current.y) < DRAG_SLOP
        ) {
          return;
        }
        if (!drag.current.moved) {
          drag.current.moved = true;
          document.body.style.cursor = "grabbing";
          document.body.style.userSelect = "none";
        }
        const target = nearestEdge(rect, e.clientX, e.clientY, fitsRef.current);
        dropTargetRef.current = target;
        setDropTarget(target);
      }
    };

    const onMove = (e: PointerEvent) => {
      if (!drag.current && !resize.current) return;
      pending = { clientX: e.clientX, clientY: e.clientY };
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        if (pending) apply(pending);
      });
    };

    const onUp = () => {
      // Drop any queued move, so a stale coordinate cannot re-open the drop
      // preview after the pointer is already up. What was previewed is what
      // gets committed.
      if (frame) {
        cancelAnimationFrame(frame);
        frame = 0;
      }
      pending = null;
      if (resize.current) {
        resize.current = null;
        setResizing(false);
      }
      if (drag.current) {
        if (drag.current.moved && dropTargetRef.current) {
          setPrefs({ dock: dropTargetRef.current });
        }
        dropTargetRef.current = null;
        setDropTarget(null);
        drag.current = null;
      }
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
    return () => {
      if (frame) cancelAnimationFrame(frame);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };
  }, [containerRef, setPrefs]);

  return {
    // The side as drawn, so the flex direction, the divider and the panel's
    // own border all agree with each other and with the size above.
    dock: drawnDock,
    height,
    width,
    size: drawn,
    resizing,
    dropTarget,
    startDockDrag,
    startResize,
  };
}
