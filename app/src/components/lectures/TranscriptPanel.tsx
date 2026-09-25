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
import { fmtClockSecs, type Cue } from "@/lib/lectures";
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

/** The dock's tabs. Enhanced text is a register of the transcript (see
 *  `TranscriptModePicker`), not a tab of its own. */
const TABS: ReadonlyArray<ViewTab<DockTab>> = [
  { value: "chapters", label: "Chapters" },
  { value: "transcript", label: "Transcript" },
  { value: "chat", label: "Chat" },
];

/** The tab really in front: the preference, unless it is a transcript the
 *  recording lacks (the strip drops that tab; the preference survives). */
export function tabInFront(tab: DockTab, hasTranscript: boolean): DockTab {
  return !hasTranscript && tab === "transcript" ? "chapters" : tab;
}

/** The register really in front: `enhanced` falls back to the cues when there
 *  is no enhanced copy, unless a run is filling it now. */
function modeInFront(
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
  /** The Chapters tab's props as one memoised bag, not a rendered node: this
   *  component is `memo`'d against a player that re-renders on every
   *  `timeupdate`, which keeps the virtualised list from re-rendering. */
  chapters: ChaptersPanelProps;
  /** The Enhanced register's bag, memoised likewise. The mode and the picker
   *  are built here, not carried. */
  reading: Omit<ReadingListProps, "picker">;
  /** The Chat tab's bag, memoised likewise; the playhead arrives as a ref. */
  chat: LectureChatPanelProps;
  dock: Dock;
  size: number;
  /** Shown or hidden — the panel stays mounted either way and slides. */
  open: boolean;
  /** Mid resize-drag: the size is following a pointer, so it must not ease. */
  resizing: boolean;
  onSeek: (seconds: number) => void;
  /** Fold the dock away. The control bar fades with the video, so the header
   *  needs its own close. */
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
 * The dock: its box and slide, the header (drag handle and tab strip), and the
 * tab in front. The Transcript tab maps cue space to `FollowList` row space.
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
  // The outer box animates one dimension to zero while the inner keeps its size,
  // so content is clipped rather than reflowed.
  const outer: CSSProperties = isVertical(dock)
    ? { height: open ? size : 0, minHeight: open ? size : 0, maxHeight: open ? size : 0 }
    : { width: open ? size : 0, minWidth: open ? size : 0, maxWidth: open ? size : 0 };
  const inner: CSSProperties = isVertical(dock)
    ? { height: size, minHeight: size }
    : { width: size, minWidth: size };

  // Not armed by an effect when `open` flips: that runs after the paint that
  // already resized the box, so the panel would snap.
  const sliding = !resizing;

  // The Transcript tab is dropped when there are no cues; Chat never is.
  const hasTranscript = cues.length > 0;
  // `TABS` is the vocabulary; the order is the reader's. `orderDockTabs` appends
  // any tab missing from the stored order.
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

  // Read from the store here so the player's memoised bags need not carry it.
  const storedMode = usePlayerPrefs((p) => p.transcriptMode);
  const setMode = useCallback(
    (transcriptMode: TranscriptMode) => setPrefs({ transcriptMode }),
    [setPrefs],
  );
  const mode = modeInFront(storedMode, reading.lines.length > 0, reading.status === "running");

  // The picker is also the enhanced copy's only Write button, so it carries the
  // job's state. Memoised: `ReadingList` is memoised against it.
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

  // The strip scrolls sideways with no visible bar; fades show the overflow.
  const stripRef = useRef<HTMLDivElement>(null);
  const [stripEdges, setStripEdges] = useState({ left: false, right: false });

  const readStripEdges = useCallback(() => {
    const el = stripRef.current;
    if (!el) return;
    const left = el.scrollLeft > 1;
    const right = el.scrollLeft + el.clientWidth < el.scrollWidth - 1;
    setStripEdges((e) => (e.left === left && e.right === right ? e : { left, right }));
  }, []);

  // Resizing, re-docking or losing a tab changes what fits; the header lays out
  // after first paint, so the initial read comes from the observer.
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

  // The list is a window onto `rows`, not `cues`: searching makes row and cue
  // indexes differ, and `rows[i]` is the only bridge.
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

  // Search suspends the follow-scroll and the pill.
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
        {/* The tabs and close button stop pointerdown: the header's dock drag
            captures the pointer, retargeting pointerup so a click never lands. */}
        <div
          onPointerDown={onHeaderPointerDown}
          className="px-2 h-9 flex items-center gap-1.5 border-b border-border shrink-0 cursor-grab active:cursor-grabbing select-none"
        >
          <Tooltip>
            <TooltipTrigger asChild>
              {/* Lifted to the tabs' text, which sits above centre by half the
                  underline's padding. */}
              <span className="mb-2 flex items-center text-muted-foreground hover:text-foreground transition-colors">
                <DotsSixVertical size={12} className="opacity-50" />
              </span>
            </TooltipTrigger>
            <TooltipContent>Drag to dock left, right, top or bottom</TooltipContent>
          </Tooltip>
          {/* No visible scrollbar (classic scrollbars here); the fades carry it. */}
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
            {/* The picker shares the search row: the dock can be 220px wide. */}
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
              // Keyed by cue index so measured heights survive filtering.
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
                      {fmtClockSecs(Math.floor(cue.start))}
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

/** The band the panel would land in, previewed mid-drag in the brand tint. */
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

/** Fade over one end of the header's tab strip while there is more that way;
 *  same gradient as `FollowList`'s, narrower because it covers a word. */
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
