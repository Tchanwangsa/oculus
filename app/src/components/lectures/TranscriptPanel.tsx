import { memo, useCallback, useMemo, type ReactNode } from "react";
import type { ViewTab } from "@/components/ui/table/ViewTabs";
import { useTranscriptSearch } from "@/hooks/lectures/useTranscriptSearch";
import {
  reorderDockTabs,
  usePlayerPrefs,
  type Dock,
  type DockTab,
} from "@/stores/lectures/playerPrefsStore";
import type { Cue } from "@/lib/lectures/media";
import { MediaDock } from "@/components/media/MediaDock";
import { TranscriptList } from "@/components/media/TranscriptList";
import {
  TranscribeEmpty,
  type TranscribeEmptyProps,
} from "@/components/media/TranscribeEmpty";
import { ChaptersPanel, type ChaptersPanelProps } from "@/components/lectures/ChaptersPanel";
import {
  LectureChatPanel,
  type LectureChatPanelProps,
} from "@/components/lectures/LectureChatPanel";

/** The dock's tabs. */
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
  /** The Chat tab's bag, memoised likewise; the playhead arrives as a ref. */
  chat: LectureChatPanelProps;
  /** The Transcript tab of a lecture without a transcript, memoised likewise;
   *  null when it has one. */
  transcribe: TranscribeEmptyProps | null;
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
 * The lecture's dock (`MediaDock`) and its three tabs: Chapters, the
 * transcript and Chat.
 */
export const TranscriptPanel = memo(function TranscriptPanel({
  cues,
  activeCueIdx,
  tab,
  onTabChange,
  chapters,
  chat,
  transcribe,
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
  // The Transcript tab is dropped while a transcript on disk has no cues
  // loaded; a lecture without one keeps it for Transcribe. Chat never is.
  const hasTranscript = cues.length > 0 || transcribe != null;
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

  // Held here, not in the list, so a query outlives a visit to another tab.
  const search = useTranscriptSearch(cues, activeCueIdx);

  let body: ReactNode;
  if (activeTab === "chat") body = <LectureChatPanel {...chat} />;
  else if (activeTab === "chapters") body = <ChaptersPanel {...chapters} />;
  else if (cues.length === 0 && transcribe) body = <TranscribeEmpty {...transcribe} />;
  else {
    body = (
      <TranscriptList
        cues={cues}
        activeCueIdx={activeCueIdx}
        search={search}
        open={open}
        active={activeTab === "transcript"}
        onSeek={onSeek}
        following={following}
        onScrollAway={onScrollAway}
        onBackToLive={onBackToLive}
      />
    );
  }

  return (
    <MediaDock
      tabs={tabs}
      value={activeTab}
      onChange={onTabChange}
      onReorder={onReorder}
      dock={dock}
      size={size}
      open={open}
      resizing={resizing}
      onClose={onClose}
      onHeaderPointerDown={onHeaderPointerDown}
    >
      {body}
    </MediaDock>
  );
});
