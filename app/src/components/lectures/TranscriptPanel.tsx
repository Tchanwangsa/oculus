import {
  memo,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import { DotsSixVertical, X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ViewTabs, type ViewTab } from "@/components/ui/ViewTabs";
import {
  isVertical,
  reorderDockTabs,
  usePlayerPrefs,
  type Dock,
  type DockTab,
  type TranscriptMode,
} from "@/stores/playerPrefsStore";
import { fmtTime, type Cue } from "@/lib/lectures";
import { FollowList, Highlight, SearchField } from "@/components/lectures/FollowList";
import { ChaptersPanel, type ChaptersPanelProps } from "@/components/lectures/ChaptersPanel";
import { ReadingList, type ReadingListProps } from "@/components/lectures/ReadingList";
import {
  TranscriptModePicker,
  type TranscriptModePickerProps,
} from "@/components/lectures/TranscriptModePicker";
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

/** The dock's three readings of the recording. No "In this video" label over
 *  them: the panel is narrow and its subject is never in doubt.
 *
 *  Chapters sits first because it is the shape of the hour — twelve rows you
 *  can take in at once, which is the one thing a six-hundred-row list cannot
 *  do. Transcript is the words, in either register (see
 *  `TranscriptModePicker`). Chat is what you reach for when neither is the
 *  question.
 *
 *  These two were briefly one tab called *Read*, with the chapters drawn as
 *  headings over the reading copy. It cost the table of contents — twelve
 *  headings scattered through six hundred lines are not an outline — and the
 *  enhanced text is a register of the transcript rather than a fourth thing,
 *  so it moved into that tab and the chapter list came back whole. */
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

/**
 * Which register is really in front, by the same argument one level down.
 *
 * `transcriptMode` is a habit carried between lectures, and most lectures have
 * no enhanced copy — so a stored `enhanced` on a recording that has none would
 * open on an empty panel with a button in it. It falls back to the cues
 * instead, and the picker is where the copy gets asked for. A run already in
 * flight keeps the enhanced view, because its lines land into it window by
 * window and watching that arrive is the point.
 */
export function modeInFront(
  mode: TranscriptMode,
  hasLines: boolean,
  running: boolean,
): TranscriptMode {
  return mode === "enhanced" && !hasLines && !running ? "standard" : mode;
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
   * A bag rather than a dozen loose props, and a value rather than a rendered
   * node: this component is `memo`'d against a player that re-renders four
   * times a second on `timeupdate`, and a fresh element on every one of those
   * would throw the memo away — which is the whole reason the virtualised list
   * is not re-rendering constantly next to a decoding video.
   */
  chapters: ChaptersPanelProps;
  /** Everything the Transcript tab's Enhanced register draws, as a second bag
   *  beside `chapters` and for the same reason. The register itself is a
   *  stored preference this panel reads, so the bag does not carry it — and
   *  nor does it carry the picker, which this panel builds from the rest. */
  reading: Omit<ReadingListProps, "picker">;
  /** Everything the Chat tab draws, as a third memoised bag beside the other
   *  two and for the same reason — read their comment. Nothing
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
 * The dock: its box and slide, the header that is both drag handle and tab
 * strip, and the tab in front. The Transcript tab is a search row over a
 * `FollowList` — of cue buttons in its Standard register, and of the reading
 * copy's lines in Enhanced (`ReadingList`), which is a register and not a tab
 * because it is the same recording, the same order and the same seek on
 * click. The virtualizer, the follow-scroll and the Back-to-live pill all
 * live in `FollowList`, which both registers share; what is left here is the
 * mapping from cue space to row space.
 */
export const TranscriptPanel = memo(function TranscriptPanel({
  cues,
  activeCueIdx,
  tab,
  onTabChange,
  chapters,
  reading,
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
  // `TABS` is the vocabulary; the order is the reader's, dragged in the header
  // and stored with the dock's side and size. A tab missing from the stored
  // order still appears — `orderDockTabs` appends it — so shipping a fourth one
  // does not need a migration.
  const order = usePlayerPrefs((p) => p.dockTabOrder);
  const setPrefs = usePlayerPrefs((p) => p.set);
  const tabs = useMemo(() => {
    const byValue = new Map(TABS.map((t) => [t.value, t]));
    const all = order.map((v) => byValue.get(v)!).filter(Boolean);
    return hasTranscript ? all : all.filter((t) => t.value !== "transcript");
  }, [order, hasTranscript]);
  const onReorder = useCallback(
    (next: DockTab[]) => setPrefs({ dockTabOrder: reorderDockTabs(order, next) }),
    [order, setPrefs],
  );
  const activeTab: DockTab = tabInFront(tab, hasTranscript);

  // Which register the Transcript tab is in. A preference like the tab itself
  // and read from the store here rather than threaded down from the player:
  // nothing above this panel needs to know, and the player's memoised bags
  // would have to be rebuilt to carry it.
  const storedMode = usePlayerPrefs((p) => p.transcriptMode);
  const setMode = useCallback(
    (transcriptMode: TranscriptMode) => setPrefs({ transcriptMode }),
    [setPrefs],
  );
  const mode = modeInFront(storedMode, reading.lines.length > 0, reading.status === "running");

  // The picker is the register switch *and* the enhanced copy's only Write
  // button, so it needs the job's state as well as the mode. Memoised because
  // both registers take it as a prop and `ReadingList` is memoised against it.
  const picker: TranscriptModePickerProps = useMemo(
    () => ({
      value: mode,
      onChange: setMode,
      status: reading.status,
      error: reading.error,
      progress: reading.progress,
      busy: reading.busy,
      downloaded: reading.downloaded,
      hasLines: reading.lines.length > 0,
      onEnhance: reading.onWrite,
    }),
    [
      mode,
      setMode,
      reading.status,
      reading.error,
      reading.progress,
      reading.busy,
      reading.downloaded,
      reading.lines.length,
      reading.onWrite,
    ],
  );

  // ── The strip's own overflow ─────────────────────────────────────────────

  // Three tabs are a squeeze in a 220px dock, so the strip scrolls sideways —
  // and a scroller with no visible bar has to say so some other way. Same shape as
  // the transcript's vertical fades below: only shown over content they are
  // actually hiding.
  const stripRef = useRef<HTMLDivElement>(null);
  const [stripEdges, setStripEdges] = useState({ left: false, right: false });

  const readStripEdges = useCallback(() => {
    const el = stripRef.current;
    if (!el) return;
    const left = el.scrollLeft > 1;
    const right = el.scrollLeft + el.clientWidth < el.scrollWidth - 1;
    setStripEdges((e) => (e.left === left && e.right === right ? e : { left, right }));
  }, []);

  // The dock being resized, docked to another edge, or losing its Transcript
  // tab all change what fits without anyone scrolling — and the header is laid
  // out after the first paint, so the initial read has to come from the
  // observer rather than from an effect that runs once.
  useEffect(() => {
    const el = stripRef.current;
    if (!el) return;
    const ro = new ResizeObserver(readStripEdges);
    ro.observe(el);
    if (el.firstElementChild) ro.observe(el.firstElementChild);
    return () => ro.disconnect();
  }, [readStripEdges]);
  useEffect(() => {
    readStripEdges();
  }, [readStripEdges, tabs, size, dock, open]);

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
          {/* Three tabs are a squeeze in a 220px dock, so the strip scrolls
              sideways rather than pushing the close button off the header. No
              visible scrollbar: these are classic scrollbars on this machine,
              and a bar under three words is furniture the header has no room
              for — the fades either side carry the affordance instead. */}
          <div className="relative min-w-0">
            <div
              ref={stripRef}
              onScroll={readStripEdges}
              className="min-w-0 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
              onPointerDown={(e) => e.stopPropagation()}
            >
              <ViewTabs
                tabs={tabs}
                value={activeTab}
                onChange={onTabChange}
                onReorder={onReorder}
                className="w-max gap-3"
              />
            </div>
            <StripFade side="left" show={stripEdges.left} />
            <StripFade side="right" show={stripEdges.right} />
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
        ) : mode === "enhanced" ? (
          <ReadingList {...reading} picker={picker} />
        ) : (
          <>
            {/* The picker shares the search row rather than taking one of its
                own: the dock is 220px at its narrowest and a second row here
                is a row of the transcript. The field takes what is left. */}
            <div className="flex shrink-0 items-center gap-1.5 px-1.5 pt-1.5">
              <SearchField
                value={query}
                onChange={setQuery}
                placeholder="Search"
                count={searching ? rows.length : undefined}
                className="min-w-0 flex-1 shrink p-0"
              />
              <TranscriptModePicker {...picker} />
            </div>
            <FollowList
              count={rows.length}
              followIdx={followIdx}
              // Keyed by *cue* index, not row: a row's measured height then
              // survives the query that moved it to a different row.
              getItemKey={(i) => rows[i]}
              open={open}
              active={activeTab === "transcript"}
              following={following}
              onScrollAway={onScrollAway}
              onBackToLive={onBackToLive}
              resetKey={needle}
              overlay={searching && rows.length === 0 ? "No matches" : undefined}
              renderRow={(row, item, measure) => {
                const cueIdx = rows[row];
                const cue = cues[cueIdx];
                const active = cueIdx === activeCueIdx;
                return (
                  <button
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
              }}
            />
          </>
        )}
      </div>
    </div>
  );
});

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

/**
 * Fade over one end of the header's tab strip, shown only while there is more
 * of it that way.
 *
 * A gradient and not a `backdrop-filter`, for the reason the transcript's own
 * fades give at length further up: a blur layer inside a panel that clips its
 * overflow and transitions its width is the compositing this player spends its
 * effort avoiding, and over the header's flat `background` the two are
 * indistinguishable anyway. It ends at `background/0` rather than
 * `transparent` for the same reason as well — `transparent` is transparent
 * *black*, and interpolating to it drags the middle of the ramp grey and
 * smears the tab label underneath.
 *
 * Narrower than the list's fades (24px against 40) because it is covering a
 * word, not a paragraph: any more and the first tab is half-dissolved before
 * the strip has been scrolled at all.
 */
function StripFade({ side, show }: { side: "left" | "right"; show: boolean }) {
  return (
    <div
      aria-hidden
      className={cn(
        "pointer-events-none absolute inset-y-0 w-6 transition-opacity duration-150",
        side === "left"
          ? "left-0 bg-gradient-to-r from-background from-15% via-background/50 via-50% to-background/0"
          : "right-0 bg-gradient-to-l from-background from-15% via-background/50 via-50% to-background/0",
        show ? "opacity-100" : "opacity-0",
      )}
    />
  );
}
