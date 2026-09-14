import { useCallback, useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";

/**
 * The thread as a rail of ticks in the left gutter — one per question asked,
 * in order. It answers "how far in am I, and what came before", which a long
 * thread otherwise only tells you by scrolling: the replies are long and the
 * questions are the only landmarks in them. Clicking a tick jumps to that
 * question; hovering names it.
 *
 * **The ticks are one block, evenly spaced — not a scale.** Placing them
 * proportionally to where their message sits in the scroll spread them over
 * the whole viewport, so a thread read as two far-apart clusters of dashes
 * rather than as one index of the conversation. Even spacing gives up "how far
 * apart" — which the scrollbar already says — and keeps the count and the
 * position, which is what the rail is for.
 *
 * It reads geometry, never the stream. Where each question actually starts is
 * kept in a ref for the jump and the active tick, so a turn that grows the
 * thread twenty times a second re-renders nothing here.
 */

export type ThreadMarker = { id: number; text: string };

const PAD = 20;
/** Distance between two ticks, and the shrunken floor for a very long thread. */
const PITCH = 9;
const MIN_PITCH = 4;
/** The timeline's own width — the rail needs a gutter outside it. */
const COLUMN = 760;
/** Where a question that is not itself on screen counts as the one you are
 *  reading: far enough down that its reply, not its own bubble, fills the view. */
const ACTIVE_LINE = 0.35;

export function ThreadMap({
  scrollRef,
  contentRef,
  markers,
}: {
  scrollRef: React.RefObject<HTMLDivElement | null>;
  contentRef: React.RefObject<HTMLDivElement | null>;
  markers: ThreadMarker[];
}) {
  const [box, setBox] = useState({ height: 0, room: false });
  // Which questions are on screen, as a range — they are in order, so what is
  // in frame is always contiguous.
  const [active, setActive] = useState({ from: 0, to: 0 });
  const [hover, setHover] = useState<number | null>(null);
  // Where each question starts in the scroller. Out of state on purpose:
  // clicking reads it, drawing does not.
  const tops = useRef<number[]>([]);
  const bottoms = useRef<number[]>([]);
  const frame = useRef(0);

  const measure = useCallback(() => {
    const el = scrollRef.current;
    const content = contentRef.current;
    if (!el || !content) return;
    const base = el.getBoundingClientRect().top - el.scrollTop;
    const t: number[] = [];
    const b: number[] = [];
    for (const m of markers) {
      const node = content.querySelector<HTMLElement>(`[data-msg-id="${m.id}"]`);
      const r = node?.getBoundingClientRect();
      t.push(r ? r.top - base : 0);
      b.push(r ? r.bottom - base : 0);
    }
    tops.current = t;
    bottoms.current = b;
    const height = Math.max(0, el.clientHeight - PAD * 2);
    // The rail lives in the gutter beside the column; a window too narrow to
    // have one would put ticks on top of the text.
    const room = (el.clientWidth - COLUMN) / 2 >= 44;
    setBox((prev) => (prev.height === height && prev.room === room ? prev : { height, room }));
  }, [scrollRef, contentRef, markers]);

  useEffect(() => {
    const el = scrollRef.current;
    const content = contentRef.current;
    if (!el || !content) return;
    measure();
    // Every question on screen is lit, not just one: a viewport holding three
    // of them and marking one says the other two are somewhere else. When none
    // is on screen — a long reply filling the view — the question that reply
    // answers is lit instead, so the rail always says where you are.
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
    // The thread grows as the turn streams and as detail cards open; both move
    // every question under them. Coalesced into a frame, and cheap: the ticks
    // themselves do not depend on what it finds.
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

  // One question is no map.
  if (!box.room || markers.length < 2) return null;

  const pitch = Math.max(MIN_PITCH, Math.min(PITCH, box.height / (markers.length - 1)));
  const blockH = pitch * (markers.length - 1);
  const start = Math.max(0, (box.height - blockH) / 2);

  const jump = (i: number) => {
    const el = scrollRef.current;
    if (el) el.scrollTo({ top: Math.max(0, tops.current[i] - 16), behavior: "smooth" });
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
                  "block h-[1.5px] rounded-full transition-all duration-150",
                  on ? "w-4 bg-primary" : "w-2.5 bg-surface-overlay group-hover/map:bg-ink-500",
                  hover === i && !on && "w-4 bg-muted-foreground",
                )}
              />
            </button>
          );
        })}

        {hover != null && (
          <div
            className="pointer-events-none absolute left-9 z-20 max-w-[240px] -translate-y-1/2 truncate rounded-md border border-border bg-popover px-2 py-1 text-[11.5px] text-popover-foreground shadow-sm"
            style={{ top: start + hover * pitch }}
          >
            {markers[hover].text.split("\n")[0] || "Message"}
          </div>
        )}
      </div>
    </div>
  );
}
