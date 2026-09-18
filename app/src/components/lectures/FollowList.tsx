/**
 * A virtualised list that follows playback.
 *
 * This is the transcript's scroller, lifted out of `TranscriptPanel` so the
 * Read tab — the same recording as one sentence per line, ~600 rows — can be
 * the second list to use it rather than a second copy of it. Everything that
 * was hard-won in the transcript lives here and only here: the snap on first
 * sync, the 20–80% band rule that keeps text from sliding under the eye, the
 * nudge / unfollow / soft-resume handover, the 8 s idle re-sync with its
 * countdown ring on the Back-to-live pill, the edge fades, and the reopen
 * scroll after the dock slides. The caller decides what a row *is*: it hands
 * over a count, a key per row, which row playback is at, and a `renderRow`
 * that draws one — and the list never learns whether that row is a cue or a
 * line.
 *
 * **Virtualised, and it has to be.** A 2-hour lecture is ~2500 cues; rendered
 * in full that is over 12,000 nodes for WebKit to lay out and paint in a
 * scroller that sits next to a decoding video, which was the floor on how
 * smooth scrolling could get no matter how little React did. Windowed, the
 * list is ~40 rows and the cost stops scaling with the lecture's length.
 *
 * Rows are variable height (a cue wraps to two lines often enough), so heights
 * are measured rather than assumed — `estimateSize` only has to be close.
 */
import {
  Fragment,
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import {
  ArrowLineDown,
  ArrowLineUp,
  MagnifyingGlass,
  X,
} from "@phosphor-icons/react";
import { useVirtualizer, type VirtualItem } from "@tanstack/react-virtual";
import { cn } from "@/lib/utils";

/** A one-line cue at the panel's default width; two-liners are measured. */
const ESTIMATED_ROW = 26;

/** The dock's slide duration, matched to the sidebar's collapse so the app has
 *  one feel. The panel's box transitions with `duration-200`; that class and
 *  this number are the same 200 ms, and the reopen-scroll below waits it out. */
const SLIDE_MS = 200;

/** How long the list has to sit untouched before it re-syncs to playback. */
const IDLE_RESYNC_MS = 8000;

/** The countdown ring drawn on the pill, in px. Hairline, like every border. */
const RING_STROKE = 1.5;

export interface FollowListProps {
  /** Rows the caller has decided to show. */
  count: number;
  /** Row index playback is at; -1 = none, which also hides the pill. */
  followIdx: number;
  /**
   * The key the virtualizer keeps a row's measured height under. Key by what
   * the row *is* (a cue index), not by its position in the list: see the
   * `resetKey` effect for what that buys.
   */
  getItemKey: (row: number) => string | number;
  /** A one-line row at the panel's default width; the rest are measured. */
  estimateSize?: number;
  /**
   * Draw one row. The element returned must be positioned by `item.start`
   * (`position: absolute; top: 0; transform: translateY(...)`) and carry
   * `data-index={item.index}` with `ref={measure}`, which is how the
   * virtualizer learns its real height.
   */
  renderRow: (
    row: number,
    item: VirtualItem,
    measure: (el: HTMLElement | null) => void,
  ) => ReactNode;
  /** The dock is open — the panel stays mounted either way and slides. */
  open: boolean;
  /** This tab is in front; drives the reopen-scroll. */
  active: boolean;
  /** The list is tracking playback rather than being read by hand. */
  following: boolean;
  /** A hand-scroll pushed the playing row out of frame — stop following. */
  onScrollAway: () => void;
  /** Resume following and snap back to the playing row. */
  onBackToLive: () => void;
  /**
   * A change arms the snap; a truthy value also scrolls to the top. The search
   * needle: results start from the top, and clearing the query hands the list
   * back to the follow-scroll, which knows better where to put it.
   */
  resetKey?: unknown;
  /** Drawn over the list, under the top fade — "No matches". */
  overlay?: ReactNode;
  className?: string;
}

export function FollowList({
  count,
  followIdx,
  getItemKey,
  estimateSize = ESTIMATED_ROW,
  renderRow,
  open,
  active,
  following,
  onScrollAway,
  onBackToLive,
  resetKey,
  overlay,
  className,
}: FollowListProps) {
  const listRef = useRef<HTMLDivElement>(null);
  /** Next follow-scroll jumps straight to the row, band or no band. */
  const snapRef = useRef(true);
  /** Read by the delayed scroll below, which fires after the panel slides. */
  const followingRef = useRef(following);
  followingRef.current = following;
  /**
   * Hand-scrolled, but the playing row is still in frame: the list holds where
   * it was put and stays live. Only the row leaving the frame ends following.
   */
  const [nudged, setNudged] = useState(false);
  const nudgedRef = useRef(false);
  /** Re-following because the row was scrolled back into view — don't snap. */
  const softResumeRef = useRef(false);
  /** Pending re-sync, pushed back by every scroll so it measures idle time. */
  const idleRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const pillRef = useRef<HTMLButtonElement>(null);
  const ringRef = useRef<SVGRectElement>(null);
  const ringAnimRef = useRef<Animation | null>(null);

  const virtualizer = useVirtualizer({
    count,
    getScrollElement: () => listRef.current,
    estimateSize: () => estimateSize,
    getItemKey,
    overscan: 12,
  });

  // Results start from the top; clearing the query hands the list back to the
  // follow-scroll, which knows better where to put it.
  //
  // Nothing re-measures here, and calling `virtualizer.measure()` would be
  // actively wrong: `getItemKey` keys the height cache by what the row *is* —
  // a cue index — so a row's measured height survives the query that moved it
  // to a different row — while `measure()` wipes the cache after the new rows
  // have already reported their heights, leaving every row on the estimate and
  // two-line rows overlapping the ones below them.
  useEffect(() => {
    snapRef.current = true;
    if (resetKey) listRef.current?.scrollTo({ top: 0, behavior: "auto" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [resetKey]);

  const items = virtualizer.getVirtualItems();

  // React 19 treats a ref callback's return value as a cleanup function, so
  // this must return nothing.
  const measure = useCallback(
    (el: HTMLElement | null) => {
      if (el) virtualizer.measureElement(el);
    },
    [virtualizer],
  );

  // ── Follow the playing row ───────────────────────────────────────────────

  // Keyed on the row index, never on `timeupdate`: re-issuing a smooth scroll
  // four times a second cancels and retargets it before it can land, so it
  // creeps forever and snatches the list back the instant you touch it.
  useEffect(() => {
    if (!open || !following || nudged || followIdx < 0) return;
    const list = listRef.current;
    if (!list) return;

    // A row the window isn't rendering is certainly off screen. One it is
    // rendering has a measured offset, so the maths below is exact.
    const item = items.find((i) => i.index === followIdx);
    const h = list.clientHeight;

    if (snapRef.current || !item) {
      // Coming back from a scroll, or from far away: let the virtualizer do
      // it — the target may never have been measured, and it corrects itself
      // once the row renders. Instant, because a smooth scroll across
      // unmeasured rows chases a moving target.
      snapRef.current = false;
      virtualizer.scrollToIndex(followIdx, { align: "center" });
      return;
    }

    // Only move once the row drifts out of the middle band, so the text is not
    // sliding under the eye on every line.
    const rel = item.start - list.scrollTop;
    if (rel >= h * 0.2 && rel + item.size <= h * 0.8) return;

    const centred = item.start - (h - item.size) / 2;
    list.scrollTo({
      top: Math.max(0, Math.min(virtualizer.getTotalSize() - h, centred)),
      behavior: "smooth",
    });
    // `items` is deliberately not a dependency: it changes on every scroll
    // frame, and this should run when the row changes, not when the window does.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [followIdx, following, nudged, virtualizer, open]);

  // Reopening lands on the playing row rather than wherever the list was when
  // it closed — and only once the box has finished growing, since a scroll
  // computed against a collapsing height ends up nowhere useful. Coming back
  // from another tab counts as reopening: the list is unmounted while that
  // tab is in front, so it returns scrolled to the top with the row index
  // unchanged, which is the one case the follow effect above cannot see.
  useEffect(() => {
    if (!open || !active) return;
    snapRef.current = true;
    const t = setTimeout(() => {
      if (followingRef.current && followIdx >= 0) {
        virtualizer.scrollToIndex(followIdx, { align: "center" });
      }
    }, SLIDE_MS + 30);
    return () => clearTimeout(t);
    // Only on open: the follow effect above owns every other reason to scroll.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, active]);

  // Arm the snap whenever following resumes, so Back to live and play both
  // land on the row rather than easing towards it. A nudge only means anything
  // while following, so either edge clears it.
  useEffect(() => {
    if (following) {
      // Except when the row is already on screen because the reader scrolled
      // it back: there is nothing to jump to, so the band rule takes over.
      snapRef.current = !softResumeRef.current;
      // Being live again is the end of the countdown, however it was reached:
      // a deliberate Back to live cancels the pending re-sync it would race,
      // and a re-sync that has already fired has nothing left to cancel.
      clearTimeout(idleRef.current);
      ringAnimRef.current?.cancel();
    }
    // The *other* edge must leave the countdown alone. Losing the row off the
    // top of the frame is what unfollows, and it is decided by the scroll
    // event *after* the last wheel tick — so clearing the timer here killed
    // the re-sync armed by that tick, and with it the ring on the pill that
    // had just appeared. The whole point of the idle timer is that this state
    // ends on its own.
    softResumeRef.current = false;
    nudgedRef.current = false;
    setNudged(false);
  }, [following]);

  useEffect(
    () => () => {
      clearTimeout(idleRef.current);
      ringAnimRef.current?.cancel();
    },
    [],
  );

  // ── Which way is live ────────────────────────────────────────────────────

  // The button points at the row, not at a fixed direction: read ahead and it
  // sends you back up, read behind and it sends you down. Measured against the
  // middle of the viewport so the answer doesn't flicker as the row crosses an
  // edge, and against the virtualizer's cache because the row is usually off
  // screen — that is why the button is showing — and so has no DOM node.
  const [liveAbove, setLiveAbove] = useState(false);

  const readDirection = useCallback(() => {
    const list = listRef.current;
    if (!list || followIdx < 0) return;
    const measured = virtualizer.measurementsCache[followIdx];
    const start = measured ? measured.start : followIdx * estimateSize;
    setLiveAbove(start < list.scrollTop + list.clientHeight / 2);
  }, [followIdx, virtualizer, estimateSize]);

  // Playback keeps moving while the list is being read by hand, so the row can
  // cross the viewport with nobody scrolling.
  useEffect(readDirection, [readDirection]);

  // ── Nudge, unfollow, re-sync ─────────────────────────────────────────────

  /** Any part of the playing row is on screen, so nothing needs to move. */
  const rowInFrame = useCallback(() => {
    const list = listRef.current;
    if (!list || followIdx < 0) return false;
    const m = virtualizer.measurementsCache[followIdx];
    const start = m ? m.start : followIdx * estimateSize;
    const size = m ? m.size : estimateSize;
    const rel = start - list.scrollTop;
    return rel + size > 0 && rel < list.clientHeight;
  }, [followIdx, virtualizer, estimateSize]);

  // The ring is drawn from the pill's own box rather than a fixed size: the
  // label is text, so its width is whatever the font renders. Measured even
  // while the pill is hidden — it is faded out, not unmounted, so it still has
  // a layout.
  const [pill, setPill] = useState({ w: 0, h: 0 });
  useEffect(() => {
    const el = pillRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      const r = el.getBoundingClientRect();
      setPill((p) => (p.w === r.width && p.h === r.height ? p : { w: r.width, h: r.height }));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // A stadium's perimeter, computed rather than asked for: `getTotalLength()`
  // on a `<rect>` is SVG2 and not worth betting a silent blank ring on.
  const ringW = Math.max(0, pill.w - RING_STROKE);
  const ringH = Math.max(0, pill.h - RING_STROKE);
  const ringLen = 2 * Math.max(0, ringW - ringH) + Math.PI * ringH;

  // Driven by hand, not by a state flag: this restarts on every scroll, and a
  // re-render per wheel tick to redraw a ring is not a trade worth making.
  const startRing = useCallback(() => {
    ringAnimRef.current?.cancel();
    const el = ringRef.current;
    if (!el || ringLen <= 0) return;
    // Dash pattern `[len on, len off]`, so the offset eats the outline from the
    // far end back to the start — full pill at zero seconds used, bare at eight.
    ringAnimRef.current = el.animate(
      [{ strokeDashoffset: 0 }, { strokeDashoffset: ringLen }],
      { duration: IDLE_RESYNC_MS, easing: "linear", fill: "forwards" },
    );
  }, [ringLen]);

  // A hand-scroll is a glance until it is proven otherwise, so the list comes
  // back on its own once it has been left alone. Every scroll pushes this back:
  // what it waits for is the hand stopping, not the first touch.
  const armResync = useCallback(() => {
    startRing();
    clearTimeout(idleRef.current);
    idleRef.current = setTimeout(() => {
      snapRef.current = true;
      nudgedRef.current = false;
      setNudged(false);
      // Following already: clearing the nudge above is what re-scrolls. Not
      // following: this is the Back to live press the user didn't have to make.
      onBackToLive();
    }, IDLE_RESYNC_MS);
  }, [onBackToLive, startRing]);

  const handleUserScroll = useCallback(() => {
    const list = listRef.current;
    // A list with nothing to scroll has nowhere to come back from.
    if (!list || virtualizer.getTotalSize() <= list.clientHeight) return;
    // Cancel any follow-scroll still animating, or it fights the wheel.
    list.scrollTo({ top: list.scrollTop, behavior: "auto" });
    // Hold the list still under the hand, but stay live: whether this scroll
    // actually left the row behind is decided once it has landed, since a wheel
    // event still reads the pre-scroll `scrollTop`.
    if (followingRef.current) {
      nudgedRef.current = true;
      setNudged(true);
    }
    armResync();
  }, [virtualizer, armResync]);

  // ── Edges ────────────────────────────────────────────────────────────────

  // Whether there is anything above or below what's on screen, so the fades
  // only appear over content they are actually hiding.
  const [edges, setEdges] = useState({ above: false, below: false });

  const readEdges = useCallback(() => {
    const list = listRef.current;
    if (!list) return;
    const above = list.scrollTop > 1;
    const below = list.scrollTop + list.clientHeight < list.scrollHeight - 1;
    setEdges((e) => (e.above === above && e.below === below ? e : { above, below }));
  }, []);

  // Rows measuring, a query narrowing the list, a resize or the panel opening
  // all change what fits without anyone scrolling. The resize is the dock's,
  // which this list knows nothing about — so it is read off the scroller's own
  // box rather than passed down as the panel's size.
  const totalSize = virtualizer.getTotalSize();
  useEffect(() => {
    readEdges();
  }, [readEdges, totalSize, count, open]);
  useEffect(() => {
    const el = listRef.current;
    if (!el) return;
    const ro = new ResizeObserver(readEdges);
    ro.observe(el);
    return () => ro.disconnect();
  }, [readEdges]);

  const handleScroll = useCallback(() => {
    readDirection();
    readEdges();
    if (nudgedRef.current) {
      // The only way out of following: a hand-scroll that pushed the row out of
      // frame. A nudge that leaves it visible needs no way back, so it is
      // offered none — the pill appearing over a list you can already read is
      // the noise this avoids.
      if (!rowInFrame()) onScrollAway();
    } else if (!followingRef.current && rowInFrame()) {
      // And the same rule in reverse: scrolling the row back into view *is* the
      // Back to live press, so it is taken as one rather than left sitting
      // under a pill pointing at a row already on screen.
      softResumeRef.current = true;
      onBackToLive();
    }
  }, [readDirection, readEdges, rowInFrame, onScrollAway, onBackToLive]);

  const showBackToLive = !following && followIdx >= 0;

  return (
    <div className={cn("relative flex-1 min-h-0", className)}>
      <div
        ref={listRef}
        // Intent, not the `scroll` event: our own follow-scroll fires scroll
        // events too, and telling the two apart after the fact is guesswork.
        // A wheel, a touch drag, or a press on the scrollbar (which lands on
        // the scroller itself, never on a row) is unambiguously the user.
        onWheel={(e) => {
          if (e.deltaY !== 0) handleUserScroll();
        }}
        onTouchMove={handleUserScroll}
        onScroll={handleScroll}
        onPointerDown={(e) => {
          if (e.target === e.currentTarget) handleUserScroll();
        }}
        className="absolute inset-0 overflow-y-auto px-1.5 py-2"
      >
        <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
          {/* Keyed here rather than trusting every caller to key the element
              it returns: the key is the virtualizer's, and a row that lost it
              would be remounted — and re-measured — on every scroll frame. */}
          {items.map((item) => (
            <Fragment key={item.key}>{renderRow(item.index, item, measure)}</Fragment>
          ))}
        </div>
      </div>

      {/* Scroll fades. A gradient rather than a `backdrop-filter`: a blur
          layer over a scrolling virtualised list that sits next to a
          decoding video is exactly the compositing the player spends its
          effort avoiding.

          Two things keep a gradient from looking like a cut. It holds
          solid `background` for its first few pixels rather than letting
          text through immediately — the top one butts against the opaque
          search row, and half-visible text a pixel under solid white
          reads as clipped, not faded — then takes the rest of its height
          to dissolve, so the hold never thickens into a white band. And
          it ends at `background/0`, not `transparent`: `transparent` is
          *transparent black*, so interpolating to it drags the middle of
          the ramp grey and leaves a dirty smear across the text. */}
      <div
        aria-hidden
        className={cn(
          "pointer-events-none absolute inset-x-0 top-0 h-10 z-10",
          "bg-gradient-to-b from-background from-15%",
          "via-background/50 via-50% to-background/0",
          "transition-opacity duration-150",
          edges.above ? "opacity-100" : "opacity-0",
        )}
      />
      <div
        aria-hidden
        className={cn(
          "pointer-events-none absolute inset-x-0 bottom-0 h-10 z-10",
          "bg-gradient-to-t from-background from-15%",
          "via-background/50 via-50% to-background/0",
          "transition-opacity duration-150",
          edges.below ? "opacity-100" : "opacity-0",
        )}
      />

      {overlay && (
        <div className="pointer-events-none absolute inset-x-0 top-6 text-center text-[11px] text-muted-foreground">
          {overlay}
        </div>
      )}

      {/* Scrolled away from the playing row — offer the way back. */}
      <div
        className={cn(
          "pointer-events-none absolute inset-x-0 bottom-3 z-20 flex justify-center transition-opacity duration-200",
          showBackToLive ? "opacity-100" : "opacity-0",
        )}
      >
        <button
          ref={pillRef}
          onClick={onBackToLive}
          tabIndex={showBackToLive ? 0 : -1}
          aria-hidden={!showBackToLive}
          className={cn(
            "pointer-events-auto relative h-6 pl-2 pr-2.5 rounded-full flex items-center gap-1",
            "bg-brand text-brand-foreground text-[11px] font-medium",
            "shadow-md shadow-black/15 hover:bg-brand-hover transition-colors",
            !showBackToLive && "pointer-events-none",
          )}
        >
          {/* Time left before the list re-syncs on its own — the outline
              drains over the eight seconds, so the pill going bare is the
              warning that it is about to jump back. */}
          {pill.w > 0 && (
            <svg
              aria-hidden
              viewBox={`0 0 ${pill.w} ${pill.h}`}
              className="pointer-events-none absolute inset-0 h-full w-full text-brand-foreground/70"
            >
              <rect
                ref={ringRef}
                x={RING_STROKE / 2}
                y={RING_STROKE / 2}
                width={ringW}
                height={ringH}
                rx={ringH / 2}
                fill="none"
                stroke="currentColor"
                strokeWidth={RING_STROKE}
                strokeDasharray={ringLen}
              />
            </svg>
          )}
          {liveAbove ? (
            <ArrowLineUp size={11} weight="bold" />
          ) : (
            <ArrowLineDown size={11} weight="bold" />
          )}
          Back to live
        </button>
      </div>
    </div>
  );
}

/**
 * The search row over a `FollowList`: a pill input with the magnifier, and —
 * while there is a query — the match count and a clear button. Escape clears
 * and blurs. The caller decides what counts as searching (`count` is shown
 * whenever it is given), since a query of nothing but spaces is not one.
 */
export function SearchField({
  value,
  onChange,
  placeholder,
  count,
  className,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  /** Matches to show beside the clear button; `undefined` when not searching. */
  count?: number;
  /** Override the row it sits in. The transcript shares its row with the
   *  Standard/Enhanced register picker, so there the field is a flex child
   *  and the
   *  padding belongs to the row instead. */
  className?: string;
}) {
  const searching = count !== undefined;
  return (
    <div className={cn("px-1.5 pt-1.5 shrink-0", className)}>
      <div className="relative">
        <MagnifyingGlass
          size={12}
          className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-muted-foreground"
        />
        <input
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              onChange("");
              e.currentTarget.blur();
            }
          }}
          placeholder={placeholder}
          spellCheck={false}
          className={cn(
            "w-full h-6 pl-6 rounded-full bg-surface text-[11px] text-foreground",
            "placeholder:text-muted-foreground focus:outline-none",
            "focus:ring-1 focus:ring-brand/40 transition-shadow",
            searching ? "pr-14" : "pr-2",
          )}
        />
        {searching && (
          <div className="absolute right-1 top-1/2 -translate-y-1/2 flex items-center gap-0.5">
            <span className="text-[10px] tabular-nums text-muted-foreground">
              {count}
            </span>
            <button
              onClick={() => onChange("")}
              aria-label="Clear search"
              className="p-0.5 rounded-full text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
            >
              <X size={10} weight="bold" />
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

/**
 * The matched run, marked in place. Split by hand rather than by a regular
 * expression: the needle is whatever was typed, so `.`, `(` and `?` are
 * characters a transcript contains, not syntax.
 */
export function Highlight({ text, needle }: { text: string; needle: string }) {
  if (!needle) return <>{text}</>;
  const hay = text.toLowerCase();
  const parts: ReactNode[] = [];
  let at = 0;
  for (;;) {
    const hit = hay.indexOf(needle, at);
    if (hit < 0) {
      parts.push(text.slice(at));
      break;
    }
    if (hit > at) parts.push(text.slice(at, hit));
    parts.push(
      <mark
        key={hit}
        className="bg-brand/20 text-brand rounded-[2px] px-px"
      >
        {text.slice(hit, hit + needle.length)}
      </mark>,
    );
    at = hit + needle.length;
  }
  return <>{parts}</>;
}
