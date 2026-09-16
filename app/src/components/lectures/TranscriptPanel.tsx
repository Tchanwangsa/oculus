import {
  memo,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import {
  ArrowLineDown,
  ArrowLineUp,
  DotsSixVertical,
  MagnifyingGlass,
  X,
} from "@phosphor-icons/react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { cn } from "@/lib/utils";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ViewTabs, type ViewTab } from "@/components/ui/ViewTabs";
import { isVertical, type Dock, type DockTab } from "@/stores/playerPrefsStore";
import { fmtTime, type Cue } from "@/lib/lectures";
import {
  ChaptersPanel,
  type ChaptersPanelProps,
} from "@/components/lectures/ChaptersPanel";
import {
  LectureChatPanel,
  type LectureChatPanelProps,
} from "@/components/lectures/LectureChatPanel";

/** Border on the panel's inner edge — the side that faces the video. */
const INNER_BORDER: Record<Dock, string> = {
  bottom: "border-t",
  top: "border-b",
  left: "border-r",
  right: "border-l",
};

/** A one-line cue at the panel's default width; two-liners are measured. */
const ESTIMATED_ROW = 26;

/** Slide duration, matched to the sidebar's collapse so the app has one feel. */
const SLIDE_MS = 200;

/** How long the list has to sit untouched before it re-syncs to playback. */
const IDLE_RESYNC_MS = 8000;

/** The countdown ring drawn on the pill, in px. Hairline, like every border. */
const RING_STROKE = 1.5;

/** The dock's three readings of the recording. No "In this video" label over
 *  them: the panel is narrow and its subject is never in doubt. */
const TABS: ReadonlyArray<ViewTab<DockTab>> = [
  { value: "chapters", label: "Chapters" },
  { value: "transcript", label: "Transcript" },
  { value: "chat", label: "Chat" },
];

/**
 * Which tab is really in front. The stored preference, unless it is the
 * transcript on a recording that has none on disk — the strip drops that tab
 * rather than offering one that could only ever be empty, and the preference
 * survives so it comes back the moment a transcript does. Exported because the
 * control bar's dock button names the same tab, and two copies of this would
 * be two answers to one question.
 */
export function tabInFront(tab: DockTab, hasTranscript: boolean): DockTab {
  return !hasTranscript && tab === "transcript" ? "chapters" : tab;
}

interface TranscriptPanelProps {
  cues: Cue[];
  activeCueIdx: number;
  /** Which tab is in front — a player preference, not a per-lecture state. */
  tab: DockTab;
  onTabChange: (tab: DockTab) => void;
  /**
   * Everything the Chapters tab draws, as one memoised bag.
   *
   * A bag rather than eight loose props, and a value rather than a rendered
   * node: this component is `memo`'d against a player that re-renders four
   * times a second on `timeupdate`, and a fresh element on every one of those
   * would throw the memo away — which is the whole reason the virtualised list
   * is not re-rendering constantly next to a decoding video.
   */
  chapters: ChaptersPanelProps;
  /** Everything the Chat tab draws, as a second memoised bag beside
   *  `chapters` and for the same reason — read that prop's comment. Nothing
   *  time-varying is in it: the playhead arrives as a ref the chip ticks
   *  itself off. */
  chat: LectureChatPanelProps;
  dock: Dock;
  size: number;
  /** Shown or hidden — the panel stays mounted either way and slides. */
  open: boolean;
  /** Mid resize-drag: the size is following a pointer, so it must not ease. */
  resizing: boolean;
  onSeek: (seconds: number) => void;
  /**
   * Fold the dock away — the header's own way out.
   *
   * The same thing the control bar's dock button and T do, offered here as
   * well because the bar is over the video and fades with it: a dock docked
   * left, on a paused lecture, is a panel whose only close control is on the
   * other side of the player.
   */
  onClose: () => void;
  /** Header press — begins the drag-to-dock gesture. */
  onHeaderPointerDown: (e: React.PointerEvent) => void;
  /** The list is tracking playback rather than being read by hand. */
  following: boolean;
  /** A hand-scroll pushed the playing cue out of frame — stop following. */
  onScrollAway: () => void;
  /** Resume following and snap back to the playing cue. */
  onBackToLive: () => void;
}

/**
 * The transcript list.
 *
 * **Virtualised, and it has to be.** A 2-hour lecture is ~2500 cues; rendered
 * in full that is over 12,000 nodes for WebKit to lay out and paint in a
 * scroller that sits next to a decoding video, which was the floor on how
 * smooth scrolling could get no matter how little React did. Windowed, the
 * list is ~40 rows and the cost stops scaling with the lecture's length.
 *
 * Rows are variable height (a cue wraps to two lines often enough), so heights
 * are measured rather than assumed — `ESTIMATED_ROW` only has to be close.
 */
export const TranscriptPanel = memo(function TranscriptPanel({
  cues,
  activeCueIdx,
  tab,
  onTabChange,
  chapters,
  chat,
  dock,
  size,
  open,
  resizing,
  onSeek,
  onClose,
  onHeaderPointerDown,
  following,
  onScrollAway,
  onBackToLive,
}: TranscriptPanelProps) {
  // The sidebar's shape: the outer box animates its one dimension to zero
  // while the inner keeps its full size, so the content is clipped rather than
  // reflowed — a transcript re-wrapping every line on the way out is what a
  // width transition looks like without it.
  const outer: CSSProperties = isVertical(dock)
    ? { height: open ? size : 0, minHeight: open ? size : 0, maxHeight: open ? size : 0 }
    : { width: open ? size : 0, minWidth: open ? size : 0, maxWidth: open ? size : 0 };
  const inner: CSSProperties = isVertical(dock)
    ? { height: size, minHeight: size }
    : { width: size, minWidth: size };

  // The transition is on except mid-drag. It cannot be *armed* by an effect
  // when `open` flips: the effect runs after the paint that already moved the
  // box to its new size, so the class arrives with nothing left to animate and
  // the panel snaps. Only rapid toggling made it look like it worked — the
  // second toggle inherited the class the first one turned on.
  const sliding = !resizing;

  // A lecture can have chapters and no transcript on disk, and a tab that
  // could only ever be empty is not a tab — so the strip shows what this
  // recording actually has. The preference survives it: the tab comes back the
  // moment a transcript does. Chat is never filtered out: it needs neither a
  // transcript nor a job that has been run, so it is the one tab every
  // recording always has — which is also what makes the dock itself
  // unconditional (`hasDock` in `LecturePlayer`).
  const hasTranscript = cues.length > 0;
  const tabs = hasTranscript ? TABS : TABS.filter((t) => t.value !== "transcript");
  const activeTab: DockTab = tabInFront(tab, hasTranscript);

  const listRef = useRef<HTMLDivElement>(null);
  /** Next follow-scroll jumps straight to the cue, band or no band. */
  const snapRef = useRef(true);
  /** Read by the delayed scroll below, which fires after the panel slides. */
  const followingRef = useRef(following);
  followingRef.current = following;
  /**
   * Hand-scrolled, but the playing cue is still in frame: the list holds where
   * it was put and stays live. Only the cue leaving the frame ends following.
   */
  const [nudged, setNudged] = useState(false);
  const nudgedRef = useRef(false);
  /** Re-following because the cue was scrolled back into view — don't snap. */
  const softResumeRef = useRef(false);
  /** Pending re-sync, pushed back by every scroll so it measures idle time. */
  const idleRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const pillRef = useRef<HTMLButtonElement>(null);
  const ringRef = useRef<SVGRectElement>(null);
  const ringAnimRef = useRef<Animation | null>(null);

  // ── Search ───────────────────────────────────────────────────────────────

  // The list is a window onto `rows`, not onto `cues`: searching narrows it to
  // the matches, so a row index and a cue index stop being the same number.
  // Everything the virtualizer is told is in row space; everything about
  // playback is in cue space, and `rows[i]` is the only bridge.
  const [query, setQuery] = useState("");
  const needle = query.trim().toLowerCase();
  const searching = needle.length > 0;

  const rows = useMemo(() => {
    if (!needle) return cues.map((_, i) => i);
    const out: number[] = [];
    for (let i = 0; i < cues.length; i++) {
      if (cues[i].text.toLowerCase().includes(needle)) out.push(i);
    }
    return out;
  }, [cues, needle]);

  // Following is about the playing cue's row, and while searching it may not
  // have one — so search suspends the follow-scroll and the pill rather than
  // fighting a list the query is choosing the contents of.
  const followIdx = searching ? -1 : activeCueIdx;

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => listRef.current,
    estimateSize: () => ESTIMATED_ROW,
    getItemKey: (i) => rows[i],
    overscan: 12,
  });

  // Results start from the top; clearing the query hands the list back to the
  // follow-scroll, which knows better where to put it.
  //
  // Nothing re-measures here, and calling `virtualizer.measure()` would be
  // actively wrong: `getItemKey` above keys the height cache by *cue* index, so
  // a row's measured height survives the query that moved it to a different row
  // — while `measure()` wipes the cache after the new rows have already
  // reported their heights, leaving every row on the estimate and two-line cues
  // overlapping the ones below them.
  useEffect(() => {
    snapRef.current = true;
    if (searching) listRef.current?.scrollTo({ top: 0, behavior: "auto" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [needle]);

  const items = virtualizer.getVirtualItems();

  // React 19 treats a ref callback's return value as a cleanup function, so
  // this must return nothing.
  const measure = useCallback(
    (el: HTMLElement | null) => {
      if (el) virtualizer.measureElement(el);
    },
    [virtualizer],
  );

  // ── Follow the playing cue ───────────────────────────────────────────────

  // Keyed on the cue index, never on `timeupdate`: re-issuing a smooth scroll
  // four times a second cancels and retargets it before it can land, so it
  // creeps forever and snatches the list back the instant you touch it.
  useEffect(() => {
    if (!open || !following || nudged || followIdx < 0) return;
    const list = listRef.current;
    if (!list) return;

    // A cue the window isn't rendering is certainly off screen. One it is
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

    // Only move once the cue drifts out of the middle band, so the text is not
    // sliding under the eye on every line.
    const rel = item.start - list.scrollTop;
    if (rel >= h * 0.2 && rel + item.size <= h * 0.8) return;

    const centred = item.start - (h - item.size) / 2;
    list.scrollTo({
      top: Math.max(0, Math.min(virtualizer.getTotalSize() - h, centred)),
      behavior: "smooth",
    });
    // `items` is deliberately not a dependency: it changes on every scroll
    // frame, and this should run when the cue changes, not when the window does.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [followIdx, following, nudged, virtualizer, open]);

  // Reopening lands on the playing cue rather than wherever the list was when
  // it closed — and only once the box has finished growing, since a scroll
  // computed against a collapsing height ends up nowhere useful. Coming back
  // from the Chapters tab counts as reopening: the list is unmounted while that
  // tab is in front, so it returns scrolled to the top with the cue index
  // unchanged, which is the one case the follow effect below cannot see.
  useEffect(() => {
    if (!open || activeTab !== "transcript") return;
    snapRef.current = true;
    const t = setTimeout(() => {
      if (followingRef.current && followIdx >= 0) {
        virtualizer.scrollToIndex(followIdx, { align: "center" });
      }
    }, SLIDE_MS + 30);
    return () => clearTimeout(t);
    // Only on open: the follow effect above owns every other reason to scroll.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, activeTab]);

  // Arm the snap whenever following resumes, so Back to live and play both
  // land on the cue rather than easing towards it. A nudge only means anything
  // while following, so either edge clears it.
  useEffect(() => {
    if (following) {
      // Except when the cue is already on screen because the reader scrolled
      // it back: there is nothing to jump to, so the band rule takes over.
      snapRef.current = !softResumeRef.current;
      // Being live again is the end of the countdown, however it was reached:
      // a deliberate Back to live cancels the pending re-sync it would race,
      // and a re-sync that has already fired has nothing left to cancel.
      clearTimeout(idleRef.current);
      ringAnimRef.current?.cancel();
    }
    // The *other* edge must leave the countdown alone. Losing the cue off the
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

  // The button points at the cue, not at a fixed direction: read ahead and it
  // sends you back up, read behind and it sends you down. Measured against the
  // middle of the viewport so the answer doesn't flicker as the cue crosses an
  // edge, and against the virtualizer's cache because the cue is usually off
  // screen — that is why the button is showing — and so has no DOM node.
  const [liveAbove, setLiveAbove] = useState(false);

  const readDirection = useCallback(() => {
    const list = listRef.current;
    if (!list || followIdx < 0) return;
    const measured = virtualizer.measurementsCache[followIdx];
    const start = measured ? measured.start : followIdx * ESTIMATED_ROW;
    setLiveAbove(start < list.scrollTop + list.clientHeight / 2);
  }, [followIdx, virtualizer]);

  // Playback keeps moving while the list is being read by hand, so the cue can
  // cross the viewport with nobody scrolling.
  useEffect(readDirection, [readDirection]);

  // ── Nudge, unfollow, re-sync ─────────────────────────────────────────────

  /** Any part of the playing cue is on screen, so nothing needs to move. */
  const cueInFrame = useCallback(() => {
    const list = listRef.current;
    if (!list || followIdx < 0) return false;
    const m = virtualizer.measurementsCache[followIdx];
    const start = m ? m.start : followIdx * ESTIMATED_ROW;
    const size = m ? m.size : ESTIMATED_ROW;
    const rel = start - list.scrollTop;
    return rel + size > 0 && rel < list.clientHeight;
  }, [followIdx, virtualizer]);

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
    // actually left the cue behind is decided once it has landed, since a wheel
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
  // all change what fits without anyone scrolling.
  const totalSize = virtualizer.getTotalSize();
  useEffect(() => {
    readEdges();
  }, [readEdges, totalSize, rows.length, size, open]);

  const handleScroll = useCallback(() => {
    readDirection();
    readEdges();
    if (nudgedRef.current) {
      // The only way out of following: a hand-scroll that pushed the cue out of
      // frame. A nudge that leaves it visible needs no way back, so it is
      // offered none — the pill appearing over a list you can already read is
      // the noise this avoids.
      if (!cueInFrame()) onScrollAway();
    } else if (!followingRef.current && cueInFrame()) {
      // And the same rule in reverse: scrolling the cue back into view *is* the
      // Back to live press, so it is taken as one rather than left sitting
      // under a pill pointing at a cue already on screen.
      softResumeRef.current = true;
      onBackToLive();
    }
  }, [readDirection, readEdges, cueInFrame, onScrollAway, onBackToLive]);

  const showBackToLive = !following && followIdx >= 0;

  return (
    <div
      style={outer}
      aria-hidden={!open}
      className={cn(
        "shrink-0 grow-0 overflow-hidden bg-background border-border",
        INNER_BORDER[dock],
        !open && "border-0",
        sliding &&
          (isVertical(dock)
            ? "transition-[height,min-height,max-height] duration-200 ease-out"
            : "transition-[width,min-width,max-width] duration-200 ease-out"),
      )}
    >
      <div style={inner} className="flex h-full flex-col min-h-0 min-w-0">
        {/* The header is the drag handle *and* the tab strip, so the tabs have
            to keep the pointerdown to themselves: `startDockDrag` captures the
            pointer on the element it fires from, which retargets the pointerup
            onto the header — and a click needs both to share a target, so the
            tab would never register one. Everything around them still drags,
            and the dots is the handle that says so. */}
        <div
          onPointerDown={onHeaderPointerDown}
          className="px-2 h-9 flex items-center gap-1.5 border-b border-border shrink-0 cursor-grab active:cursor-grabbing select-none"
        >
          <Tooltip>
            <TooltipTrigger asChild>
              {/* Lifted onto the tabs' text, which sits above their own centre
                  by half of the underline's padding. */}
              <span className="mb-2 flex items-center text-muted-foreground hover:text-foreground transition-colors">
                <DotsSixVertical size={12} className="opacity-50" />
              </span>
            </TooltipTrigger>
            <TooltipContent>Drag to dock left, right, top or bottom</TooltipContent>
          </Tooltip>
          <div className="min-w-0" onPointerDown={(e) => e.stopPropagation()}>
            <ViewTabs
              tabs={tabs}
              value={activeTab}
              onChange={onTabChange}
              className="gap-3"
            />
          </div>
          {/* Lifted onto the tabs' text like the drag handle is, and it keeps
              its own pointerdown for the same reason they do: the header's
              press starts a dock drag, which captures the pointer and would
              carry this button's click away with it. */}
          <button
            type="button"
            onPointerDown={(e) => e.stopPropagation()}
            onClick={onClose}
            aria-label="Hide panel"
            className="mb-2 ml-auto shrink-0 rounded-full p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          >
            <X size={11} weight="bold" />
          </button>
        </div>

        {activeTab === "chat" ? (
          <LectureChatPanel {...chat} />
        ) : activeTab === "chapters" ? (
          <ChaptersPanel {...chapters} />
        ) : (
          <>
          <div className="px-1.5 pt-1.5 shrink-0">
            <div className="relative">
              <MagnifyingGlass
                size={12}
                className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-muted-foreground"
              />
              <input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Escape") {
                    setQuery("");
                    e.currentTarget.blur();
                  }
                }}
                placeholder="Search transcript"
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
                    {rows.length}
                  </span>
                  <button
                    onClick={() => setQuery("")}
                    aria-label="Clear search"
                    className="p-0.5 rounded-full text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
                  >
                    <X size={10} weight="bold" />
                  </button>
                </div>
              )}
            </div>
          </div>

          <div className="relative flex-1 min-h-0">
            <div
              ref={listRef}
              // Intent, not the `scroll` event: our own follow-scroll fires scroll
              // events too, and telling the two apart after the fact is guesswork.
              // A wheel, a touch drag, or a press on the scrollbar (which lands on
              // the scroller itself, never on a cue) is unambiguously the user.
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
              <div
                style={{ height: virtualizer.getTotalSize(), position: "relative" }}
              >
                {items.map((item) => {
                  const cueIdx = rows[item.index];
                  const cue = cues[cueIdx];
                  const active = cueIdx === activeCueIdx;
                  return (
                    <button
                      key={item.key}
                      data-index={item.index}
                      ref={measure}
                      onClick={() => onSeek(cue.start)}
                      style={{
                        position: "absolute",
                        top: 0,
                        left: 0,
                        width: "100%",
                        transform: `translateY(${item.start}px)`,
                      }}
                      className={cn(
                        "text-left text-[11px] px-2 py-1 rounded flex gap-2 items-start",
                        active
                          ? "bg-brand/12 text-brand"
                          : "text-muted-foreground hover:text-foreground hover:bg-surface",
                      )}
                    >
                      <span className="tabular-nums text-[10px] shrink-0 pt-px w-10 opacity-60">
                        {fmtTime(Math.floor(cue.start))}
                      </span>
                      <span className="flex-1">
                        <Highlight text={cue.text} needle={needle} />
                      </span>
                    </button>
                  );
                })}
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

            {searching && rows.length === 0 && (
              <div className="pointer-events-none absolute inset-x-0 top-6 text-center text-[11px] text-muted-foreground">
                No matches
              </div>
            )}

            {/* Scrolled away from the playing cue — offer the way back. */}
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
          </>
        )}
      </div>
    </div>
  );
});

/**
 * The matched run, marked in place. Split by hand rather than by a regular
 * expression: the needle is whatever was typed, so `.`, `(` and `?` are
 * characters a transcript contains, not syntax.
 */
function Highlight({ text, needle }: { text: string; needle: string }) {
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

/**
 * The band the panel would land in, previewed under the pointer mid-drag —
 * brand-tinted rather than the usual grey, so it reads as the one accent.
 */
export function DockDropPreview({
  dock,
  height,
  width,
}: {
  dock: Dock;
  height: number;
  width: number;
}) {
  const edge: Record<Dock, CSSProperties> = {
    bottom: { left: 0, right: 0, bottom: 0, height },
    top: { left: 0, right: 0, top: 0, height },
    left: { top: 0, bottom: 0, left: 0, width },
    right: { top: 0, bottom: 0, right: 0, width },
  };

  return (
    <div className="absolute inset-0 z-40 pointer-events-none">
      <div
        style={edge[dock]}
        className="absolute rounded-sm bg-brand/25 border border-brand/60 backdrop-blur-[1px] transition-all duration-100"
      />
    </div>
  );
}

/** Divider between the video stack and the panel; drag to resize. */
export function DockResizeHandle({
  dock,
  onPointerDown,
}: {
  dock: Dock;
  onPointerDown: (e: React.PointerEvent) => void;
}) {
  const vertical = isVertical(dock);
  return (
    <div
      role="separator"
      aria-orientation={vertical ? "horizontal" : "vertical"}
      onPointerDown={onPointerDown}
      className={cn(
        "shrink-0 relative z-20 hover:bg-brand/40 active:bg-brand/60 transition-colors",
        vertical ? "h-px w-full cursor-row-resize" : "w-px h-full cursor-col-resize",
      )}
    >
      {/* Wider invisible hit area than the hairline it draws. */}
      <div
        className={cn(
          "absolute",
          vertical ? "inset-x-0 -top-1.5 -bottom-1.5" : "inset-y-0 -left-1.5 -right-1.5",
        )}
      />
    </div>
  );
}
