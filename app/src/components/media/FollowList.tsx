/**
 * A virtualised list that follows playback, behind `TranscriptList`: snap on
 * first sync, the 20–80% band rule, nudge / unfollow / soft-resume, idle
 * re-sync with a countdown ring, edge fades, reopen scroll.
 * Virtualised because a transcript runs to thousands of rows beside a decoding
 * video; heights are measured, so `estimateSize` need only be close.
 */
import {
  Fragment,
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useVirtualizer, type VirtualItem } from "@tanstack/react-virtual";
import { cn } from "@/lib/utils";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { BackToLivePill } from "@/components/media/follow/BackToLivePill";
import {
  ESTIMATED_ROW,
  IDLE_RESYNC_MS,
  SLIDE_MS,
} from "@/components/media/follow/constants";
import { useBackToLiveRing } from "@/components/media/follow/useBackToLiveRing";

export interface FollowListProps {
  count: number;
  /** Row index playback is at; -1 = none, which also hides the pill. */
  followIdx: number;
  /** Height-cache key: key by what the row *is* (a cue index), not its
   *  position — see the `resetKey` effect. */
  getItemKey: (row: number) => string | number;
  estimateSize?: number;
  /** Draw one row, positioned by `item.start` and carrying
   *  `data-index={item.index}` and `ref={measure}`. */
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
  /** A change arms the snap; a truthy value (the search needle) also scrolls
   *  to the top. */
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
  const followIdxRef = useRef(followIdx);
  followIdxRef.current = followIdx;
  /** Hand-scrolled with the playing row still in frame: hold position, stay
   *  live. Only the row leaving the frame ends following. */
  const [nudged, setNudged] = useState(false);
  const nudgedRef = useRef(false);
  /** Re-following because the row was scrolled back into view — don't snap. */
  const softResumeRef = useRef(false);
  /** Pending re-sync, pushed back by every scroll so it measures idle time. */
  const idleRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const ringAnimRef = useRef<Animation | null>(null);

  const virtualizer = useVirtualizer({
    count,
    getScrollElement: () => listRef.current,
    estimateSize: () => estimateSize,
    getItemKey,
    overscan: 12,
  });

  // No `virtualizer.measure()` here: heights are cached by row key and survive
  // a query moving them, while `measure()` would wipe them after the new rows
  // reported, leaving two-line rows overlapping.
  useEffect(() => {
    snapRef.current = true;
    if (resetKey) listRef.current?.scrollTo({ top: 0, behavior: "auto" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [resetKey]);

  const items = virtualizer.getVirtualItems();

  // React 19 treats a ref callback's return value as cleanup: return nothing.
  const measure = useCallback(
    (el: HTMLElement | null) => {
      if (el) virtualizer.measureElement(el);
    },
    [virtualizer],
  );

  // Keyed on the row index, never `timeupdate`: re-issuing a smooth scroll
  // retargets it before it lands, so it creeps forever.
  useEffect(() => {
    if (!open || !following || nudged || followIdx < 0) return;
    const list = listRef.current;
    if (!list) return;

    // An unrendered row is off screen; a rendered one has an exact offset.
    const item = items.find((i) => i.index === followIdx);
    const h = list.clientHeight;

    if (snapRef.current || !item) {
      // Far away or re-syncing: instant `scrollToIndex`, which corrects itself
      // once unmeasured rows render (a smooth scroll would chase them).
      snapRef.current = false;
      virtualizer.scrollToIndex(followIdx, { align: "center" });
      return;
    }

    // Move only once the row drifts out of the middle band.
    const rel = item.start - list.scrollTop;
    if (rel >= h * 0.2 && rel + item.size <= h * 0.8) return;

    const centred = item.start - (h - item.size) / 2;
    list.scrollTo({
      top: Math.max(0, Math.min(virtualizer.getTotalSize() - h, centred)),
      behavior: "smooth",
    });
    // Not on `items`: it changes every scroll frame.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [followIdx, following, nudged, virtualizer, open]);

  // Reopening (or returning from another tab, which remounts the list at the
  // top with the row unchanged) lands on the playing row once the slide ends.
  useEffect(() => {
    if (!open || !active) return;
    snapRef.current = true;
    const t = setTimeout(() => {
      if (followingRef.current && followIdxRef.current >= 0) {
        virtualizer.scrollToIndex(followIdxRef.current, { align: "center" });
      }
    }, SLIDE_MS + 30);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, active]);

  // Resuming follow arms the snap, and either edge clears the nudge.
  useEffect(() => {
    if (following) {
      // Unless the reader scrolled the row back into view: then the band rule.
      snapRef.current = !softResumeRef.current;
      // Live again ends the countdown, however it was reached.
      clearTimeout(idleRef.current);
      ringAnimRef.current?.cancel();
    }
    // Unfollowing must not clear the timer: it is decided by the scroll event
    // after the last wheel tick, which has just armed the re-sync.
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

  // Which way the pill points, measured from the viewport's middle against the
  // virtualizer's cache — the row is usually off screen and has no node.
  const [liveAbove, setLiveAbove] = useState(false);

  const readDirection = useCallback(() => {
    const list = listRef.current;
    if (!list || followIdx < 0) return;
    const measured = virtualizer.measurementsCache[followIdx];
    const start = measured ? measured.start : followIdx * estimateSize;
    setLiveAbove(start < list.scrollTop + list.clientHeight / 2);
  }, [followIdx, virtualizer, estimateSize]);

  // Playback moves the row even while nobody scrolls.
  useEffect(readDirection, [readDirection]);

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

  const { pillRef, ringRef, pill, ringW, ringH, ringLen, startRing } =
    useBackToLiveRing(ringAnimRef);

  // A hand-scroll is a glance: every scroll pushes the re-sync deadline back.
  const armResync = useCallback(() => {
    startRing();
    clearTimeout(idleRef.current);
    idleRef.current = setTimeout(() => {
      snapRef.current = true;
      nudgedRef.current = false;
      setNudged(false);
      // Following: clearing the nudge re-scrolls. Not following: this is the
      // Back to live press.
      onBackToLive();
    }, IDLE_RESYNC_MS);
  }, [onBackToLive, startRing]);

  const handleUserScroll = useCallback(() => {
    const list = listRef.current;
    // A list with nothing to scroll has nowhere to come back from.
    if (!list || virtualizer.getTotalSize() <= list.clientHeight) return;
    // Cancel any follow-scroll still animating, or it fights the wheel.
    list.scrollTo({ top: list.scrollTop, behavior: "auto" });
    // Stay live for now: whether the row was left behind is decided on
    // `scroll`, since a wheel event reads the pre-scroll `scrollTop`.
    if (followingRef.current) {
      nudgedRef.current = true;
      setNudged(true);
    }
    armResync();
  }, [virtualizer, armResync]);

  // Fades only over content they are actually hiding. The virtualizer's sized
  // wrapper is the first child, so measuring, filtering or a dock resize all
  // trip the hook's observer; opening re-hangs it.
  useScrollFade(listRef, "y", open);

  const handleScroll = useCallback(() => {
    readDirection();
    if (nudgedRef.current) {
      // A nudge that pushed the row out of frame is the only way out of following.
      if (!rowInFrame()) onScrollAway();
    } else if (!followingRef.current && rowInFrame()) {
      // Scrolling the row back into view is the Back to live press.
      softResumeRef.current = true;
      onBackToLive();
    }
  }, [readDirection, rowInFrame, onScrollAway, onBackToLive]);

  const showBackToLive = !following && followIdx >= 0;

  return (
    // Out of the page's DOM find: only the rows near the viewport exist, so
    // its counts would lie. The panels' own search fields cover the list.
    <div data-find-skip className={cn("relative flex-1 min-h-0", className)}>
      <div
        ref={listRef}
        // Intent, not `scroll`: our follow-scroll fires scroll events too. A press
        // on the scroller itself (never a row) is the scrollbar.
        onWheel={(e) => {
          if (e.deltaY !== 0) handleUserScroll();
        }}
        onTouchMove={handleUserScroll}
        onScroll={handleScroll}
        onPointerDown={(e) => {
          if (e.target === e.currentTarget) handleUserScroll();
        }}
        className="absolute inset-0 overflow-y-auto px-1.5 py-2 [--scroll-fade:40px]"
      >
        <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
          {/* Keyed with the virtualizer's key, so a row is never remounted and
              re-measured on scroll. */}
          {items.map((item) => (
            <Fragment key={item.key}>{renderRow(item.index, item, measure)}</Fragment>
          ))}
        </div>
      </div>

      {overlay && (
        <div className="pointer-events-none absolute inset-x-0 top-6 text-center text-[11px] text-muted-foreground">
          {overlay}
        </div>
      )}

      <BackToLivePill
        show={showBackToLive}
        liveAbove={liveAbove}
        onBackToLive={onBackToLive}
        pillRef={pillRef}
        ringRef={ringRef}
        pill={pill}
        ringW={ringW}
        ringH={ringH}
        ringLen={ringLen}
      />
    </div>
  );
}
