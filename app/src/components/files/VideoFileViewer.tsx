import { memo, useCallback, useEffect, useMemo, useState } from "react";
import type { DbFile } from "@/lib/db";
import {
  mediaSrc,
  parseVtt,
  pauseLibraryVideos,
  registerLibraryVideo,
  type Cue,
} from "@/lib/lectures/media";
import { pauseLecturePlayback } from "@/lib/lectures/playback";
import { courseFileHasContent, readCourseFile } from "@/lib/files/courseFiles";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { useTranscriptSearch } from "@/hooks/lectures/useTranscriptSearch";
import {
  TRANSCRIBED_EVENT,
  settleTranscription,
  type TranscribedDetail,
} from "@/hooks/lectures/useTranscription";
import { useTabActive } from "@/components/tabs/TabContext";
import type { ViewTab } from "@/components/ui/table/ViewTabs";
import type { Dock } from "@/stores/lectures/playerPrefsStore";
import { MediaPlayer, useMediaPlayer } from "@/components/media/MediaPlayer";
import { MediaDock } from "@/components/media/MediaDock";
import { TranscriptList } from "@/components/media/TranscriptList";
import { PanelEmpty } from "@/components/media/MediaDock";
import { TranscribeEmpty } from "@/components/media/TranscribeEmpty";

const TABS: ReadonlyArray<ViewTab<"transcript">> = [
  { value: "transcript", label: "Transcript" },
];

const noop = () => {};

/**
 * A video in the library (a Canvas module's `.mp4`, say) in the media player.
 * Its captions are the `<file>.vtt` beside it, which Transcribe writes when
 * there is none. The element is this component's own, not the lectures'.
 */
export function VideoFileViewer({ file }: { file: DbFile }) {
  const rel = file.relative_path;
  const vttRel = `${rel}.vtt`;
  const onScreen = useTabActive();

  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let stale = false;
    setSrc(null);
    mediaSrc(rel).then((url) => !stale && setSrc(url));
    return () => {
      stale = true;
    };
  }, [rel]);

  const [cues, setCues] = useState<Cue[]>([]);
  /** The sibling VTT is on disk; null until the probe answers, so Transcribe
   *  never flashes over a transcript that is loading. */
  const [hasVtt, setHasVtt] = useState<boolean | null>(null);
  /** The VTT is there but could not be read: say so, never offer Transcribe
   *  over it. */
  const [readError, setReadError] = useState<string | null>(null);

  const loadCues = useCallback(() => {
    let stale = false;
    let hasRead = false;
    const read = async () => {
      if (!(await courseFileHasContent(vttRel))) return null;
      hasRead = true;
      return parseVtt(await readCourseFile(vttRel));
    };
    read()
      .then((parsed) => {
        if (stale) return;
        setCues(parsed ?? []);
        setHasVtt(parsed != null);
        setReadError(null);
      })
      .catch((e) => {
        if (stale) return;
        setHasVtt(true);
        setReadError(`The transcript could not be read: ${e}`);
      })
      // A finished run waits, as a spinner, until a read finds its VTT.
      .finally(() => !stale && hasRead && settleTranscription(rel));
    return () => {
      stale = true;
    };
  }, [vttRel, rel]);

  useEffect(() => {
    setCues([]);
    setHasVtt(null);
    setReadError(null);
    return loadCues();
  }, [loadCues]);

  useWindowEvent(TRANSCRIBED_EVENT, (e) => {
    if ((e as CustomEvent<TranscribedDetail>).detail.path === rel) loadCues();
  });

  const player = useMediaPlayer({
    cues,
    resetKey: rel,
    dockTab: "transcript",
    hasCaptions: cues.length > 0,
  });

  const [el, setEl] = useState<HTMLVideoElement | null>(null);
  const { attach } = player;
  useEffect(() => attach(el), [el, attach]);

  // Nothing else stops this element: a lecture's play on parked, this one
  // pauses with its tab.
  useEffect(() => {
    if (!onScreen) el?.pause();
  }, [onScreen, el]);

  // One sound at a time: playing here pauses the lecture and any other
  // library video; a lecture starting pauses this one (`lib/lectures/playback/`).
  useEffect(() => {
    if (!el) return;
    const unregister = registerLibraryVideo(el);
    const onPlay = () => {
      pauseLecturePlayback();
      pauseLibraryVideos(el);
    };
    el.addEventListener("play", onPlay);
    return () => {
      el.removeEventListener("play", onPlay);
      unregister();
    };
  }, [el]);

  const { dock, size, resizing, startDockDrag } = player.dock;

  return (
    <MediaPlayer
      player={player}
      canPlay={!!src}
      previewSrc={src}
      dockLabel={`${player.dockOpen ? "Hide" : "Show"} transcript (T)`}
      dock={
        <TranscriptDock
          cues={cues}
          activeCueIdx={player.activeCueIdx}
          path={hasVtt === false && cues.length === 0 ? rel : null}
          checking={hasVtt === null}
          error={readError}
          dock={dock}
          size={size}
          open={player.dockOpen}
          resizing={resizing}
          onSeek={player.seek}
          onClose={player.closeDock}
          onHeaderPointerDown={startDockDrag}
          following={player.following}
          onScrollAway={player.onScrollAway}
          onBackToLive={player.onBackToLive}
        />
      }
    >
      <div className="relative min-h-0 min-w-0 flex-1 overflow-hidden">
        {src && (
          <video
            ref={setEl}
            src={src}
            playsInline
            preload="metadata"
            className="h-full w-full object-contain"
          />
        )}
      </div>
    </MediaPlayer>
  );
}

/** The one-tab dock. Memoised against a player that re-renders on every
 *  `timeupdate`, as the lecture's is. */
const TranscriptDock = memo(function TranscriptDock({
  cues,
  activeCueIdx,
  path,
  checking,
  error,
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
}: {
  cues: Cue[];
  activeCueIdx: number;
  /** The video to transcribe when it has no transcript; null when it has. */
  path: string | null;
  /** The sibling VTT is still being looked for. */
  checking: boolean;
  /** The sibling VTT exists but could not be read. */
  error: string | null;
  dock: Dock;
  size: number;
  open: boolean;
  resizing: boolean;
  onSeek: (seconds: number) => void;
  onClose: () => void;
  onHeaderPointerDown: (e: React.PointerEvent) => void;
  following: boolean;
  onScrollAway: () => void;
  onBackToLive: () => void;
}) {
  // Held here, not in the list, so a query outlives the list's remount.
  const search = useTranscriptSearch(cues, activeCueIdx);
  const empty = useMemo(() => (path ? { path } : null), [path]);

  return (
    <MediaDock
      tabs={TABS}
      value="transcript"
      onChange={noop}
      dock={dock}
      size={size}
      open={open}
      resizing={resizing}
      onClose={onClose}
      onHeaderPointerDown={onHeaderPointerDown}
    >
      {error ? (
        <PanelEmpty>
          <p className="text-destructive">{error}</p>
        </PanelEmpty>
      ) : empty ? (
        <TranscribeEmpty {...empty} />
      ) : checking ? null : (
        <TranscriptList
          cues={cues}
          activeCueIdx={activeCueIdx}
          search={search}
          open={open}
          active
          onSeek={onSeek}
          following={following}
          onScrollAway={onScrollAway}
          onBackToLive={onBackToLive}
        />
      )}
    </MediaDock>
  );
});
