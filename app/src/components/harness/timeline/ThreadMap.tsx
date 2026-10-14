import { memo, useCallback, useEffect, useRef, useState } from "react";
import { expandWithin } from "@/lib/ui/expandable";
import { inSkippedSubtree, SKIPPED_CONTENT, zoomScale } from "@/lib/ui/scrollAnchor";
import { cn } from "@/lib/utils";

/**
 * A rail of ticks in the left gutter, one per question; click jumps, hover names.
 * Ticks are evenly spaced, not proportional to scroll position (the scrollbar
 * already says that). Positions live in refs, so streaming re-renders nothing.
 */

export type ThreadMarker = { id: number; text: string };

const PAD = 20;
/** Distance between two ticks, and the shrunken floor for a very long thread. */
const PITCH = 9;
const MIN_PITCH = 4;
/** The timeline column's width; the rail needs a gutter outside it. */
const COLUMN = 760;
/** With no question on screen, the one above this viewport fraction is active. */
const ACTIVE_LINE = 0.35;
/** Where a jump leaves the question, below the scroller's top. */
const JUMP_GAP = 16;

export const ThreadMap = memo(function ThreadMap({
  scrollRef,
  contentRef,
  markers,
}: {
  scrollRef: React.RefObject<HTMLDivElement | null>;
  contentRef: React.RefObject<HTMLDivElement | null>;
  markers: ThreadMarker[];
}) {
  const [box, setBox] = useState({ height: 0, room: false });
  // On-screen questions are contiguous, so a range.
  const [active, setActive] = useState({ from: 0, to: 0 });
  const [hover, setHover] = useState<number | null>(null);
  const tops = useRef<number[]>([]);
  const bottoms = useRef<number[]>([]);
  const frame = useRef(0);

  const measure = useCallback(() => {
    const el = scrollRef.current;
    const content = contentRef.current;
    if (!el || !content) return;
    const height = Math.max(0, el.clientHeight - PAD * 2);
    const room = (el.clientWidth - COLUMN) / 2 >= 44;
    setBox((prev) => (prev.height === height && prev.room === room ? prev : { height, room }));
    // The rail is absent in narrow panes and single-question threads.
    if (!room || markers.length < 2) {
      tops.current = [];
      bottoms.current = [];
      return;
    }
    // Rects scale with page zoom and `scrollTop` doesn't; tops are in scroll px.
    const view = el.getBoundingClientRect();
    const scale = zoomScale(view.height, el.offsetHeight);
    const at = (y: number) => (y - view.top) / scale + el.scrollTop;
    // The live class collection includes newly mounted history without walking
    // through every reply's markdown and maths subtree on each resize frame.
    const nodes = new Map<number, HTMLElement>();
    for (const node of content.getElementsByClassName("thread-map-question-row")) {
      const question = node as HTMLElement;
      nodes.set(Number(question.dataset.questionId), question);
    }
    const t: number[] = [];
    const b: number[] = [];
    for (const m of markers) {
      const row = nodes.get(m.id);
      // A hidden bubble's rect would force its layout; its row box is cached.
      const node = row?.hasAttribute(SKIPPED_CONTENT) ? row : row?.querySelector<HTMLElement>("[data-msg-id]") ?? row;
      const r = node?.getBoundingClientRect();
      // A question the timeline hasn't mounted is above every mounted one.
      t.push(r ? at(r.top) : -Infinity);
      b.push(r ? at(r.bottom) : -Infinity);
    }
    tops.current = t;
    bottoms.current = b;
  }, [scrollRef, contentRef, markers]);

  useEffect(() => {
    const el = scrollRef.current;
    const content = contentRef.current;
    if (!el || !content) return;
    measure();
    // Light every on-screen question; if none, the one whose reply fills the view.
    const onScroll = () => {
      const top = el.scrollTop;
      const bottom = top + el.clientHeight;
      let from = -1;
      let to = -1;
      for (let i = 0; i < tops.current.length; i++) {
        if (bottoms.current[i] > top && tops.current[i] < bottom) {
          if (from < 0) from = i;
          to = i;
        }
      }
      if (from < 0) {
        const line = top + el.clientHeight * ACTIVE_LINE;
        let i = 0;
        while (i + 1 < tops.current.length && tops.current[i + 1] <= line) i++;
        from = to = i;
      }
      setActive((prev) => (prev.from === from && prev.to === to ? prev : { from, to }));
    };
    onScroll();
    el.addEventListener("scroll", onScroll, { passive: true });
    // Streaming and opening cards move questions; re-measure once per frame.
    const schedule = () => {
      if (frame.current) return;
      frame.current = requestAnimationFrame(() => {
        frame.current = 0;
        measure();
        onScroll();
      });
    };
    const ro = new ResizeObserver(schedule);
    ro.observe(content);
    ro.observe(el);
    return () => {
      el.removeEventListener("scroll", onScroll);
      ro.disconnect();
      if (frame.current) cancelAnimationFrame(frame.current);
      frame.current = 0;
    };
  }, [scrollRef, contentRef, measure]);

  if (!box.room || markers.length < 2) return null;

  const pitch = Math.max(MIN_PITCH, Math.min(PITCH, box.height / (markers.length - 1)));
  const blockH = pitch * (markers.length - 1);
  const start = Math.max(0, (box.height - blockH) / 2);

  const hovered = hover == null ? undefined : markers[hover];

  // Instant, not smooth: rows WebKit skipped draw on the way and change
  // height, and the anchoring scroll that holds the view through that would
  // cancel a smooth scroll midway. A second pass catches what they moved.
  const jump = (i: number) => {
    const el = scrollRef.current;
    const content = contentRef.current;
    const selector = `[data-msg-id="${markers[i].id}"]`;
    const current = content?.querySelector(selector);
    if (!current || inSkippedSubtree(current)) expandWithin(content, { sync: true, upToId: markers[i].id });
    const node = content?.querySelector(selector);
    if (!el || !node) return;
    const settle = () => {
      const view = el.getBoundingClientRect();
      const off = (node.getBoundingClientRect().top - view.top) / zoomScale(view.height, el.offsetHeight);
      if (Math.abs(off - JUMP_GAP) >= 1) el.scrollTop += off - JUMP_GAP;
    };
    settle();
    requestAnimationFrame(() => requestAnimationFrame(() => {
      if (node.isConnected) settle();
    }));
  };

  return (
    <div
      className="group/map pointer-events-none absolute left-1.5 top-0 z-10 w-8"
      style={{ paddingTop: PAD, height: box.height + PAD * 2 }}
    >
      <div className="relative h-full">
        {markers.map((m, i) => {
          const on = i >= active.from && i <= active.to;
          return (
            <button
              key={m.id}
              type="button"
              onClick={() => jump(i)}
              onMouseEnter={() => setHover(i)}
              onMouseLeave={() => setHover((h) => (h === i ? null : h))}
              aria-label={m.text.split("\n")[0] || "Message"}
              className="pointer-events-auto absolute left-0 flex w-8 -translate-y-1/2 items-center"
              style={{ top: start + i * pitch, height: pitch }}
            >
              <span
                className={cn(
                  "block h-[1.5px] rounded-full transition-[width,background-color] duration-150",
                  on ? "w-4 bg-primary" : "w-2.5 bg-surface-overlay group-hover/map:bg-ink-500",
                  hover === i && !on && "w-4 bg-muted-foreground",
                )}
              />
            </button>
          );
        })}

        {hover != null && hovered && (
          <div
            className="pointer-events-none absolute left-9 z-20 max-w-[240px] -translate-y-1/2 truncate rounded-md border border-border bg-popover px-2 py-1 text-[11.5px] text-popover-foreground shadow-sm"
            style={{ top: start + hover * pitch }}
          >
            {hovered.text.split("\n")[0] || "Message"}
          </div>
        )}
      </div>
    </div>
  );
});
