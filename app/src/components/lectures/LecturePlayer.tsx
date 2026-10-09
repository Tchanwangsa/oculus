import { useCallback, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { CircleNotch, DownloadSimple } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { downloadLecture } from "@/stores/lectures/lectureDownloadStore";
import { spanAt } from "@/lib/lectures/media";
import type { Lecture, SourceNum } from "@/lib/db";
import { contentEnd } from "@/lib/lectures/end";
import {
  claimPlayback,
  mayAdopt,
  playbackOwner,
  subscribePlaybackOwner,
} from "@/lib/lectures/playbackOwner";
import { usePaneTab, useTabActive, useTabId } from "@/components/tabs/TabContext";
import { useLectureChapters } from "@/hooks/lectures/useLectureChapters";
import { useLectureEnd } from "@/hooks/lectures/useLectureEnd";
import { PIP_CORNERS, useSourceLayout } from "@/hooks/lectures/useSourceLayout";
import { usePlayerPrefs, type DockTab } from "@/stores/lectures/playerPrefsStore";
import { LayoutControl } from "@/components/lectures/LayoutControl";
import { ControlButton, MediaPlayer, useMediaPlayer } from "@/components/media/MediaPlayer";
import { TranscriptPanel, tabInFront } from "@/components/lectures/TranscriptPanel";
import { DownloadPrompt } from "@/components/lectures/player/DownloadPrompt";
import { ElsewhereOverlay } from "@/components/lectures/player/ElsewhereOverlay";
import { CORNER_STYLE, DOCK_TAB_NOUN } from "@/components/lectures/player/constants";
import { StackDivider } from "@/components/lectures/player/StackDivider";
import { UpNextPanel } from "@/components/lectures/player/UpNextPanel";
import { useDockProps } from "@/components/lectures/player/useDockProps";
import { useLectureSources } from "@/components/lectures/player/useLectureSources";
import { useLectureTranscript } from "@/components/lectures/player/useLectureTranscript";
import { useSharedElement } from "@/components/lectures/player/useSharedElement";
import { VideoFrame } from "@/components/lectures/player/VideoFrame";

interface LecturePlayerProps {
  lecture: Lecture;
  /** Fired after anything persisted changes (progress, downloads). */
  onRefresh: () => void;
}

/**
 * The lecture page's player: `MediaPlayer` over the shared elements
 * (`lib/lectures/playback/`), with what only a lecture has — two sources, the
 * pane that owns playback, chapters, chat and downloads.
 */
export function LecturePlayer({ lecture, onRefresh }: LecturePlayerProps) {
  const [error, setError] = useState<string | null>(null);

  /** Every tab stays mounted and there is one `<video>` per source app-wide, so
   *  only an on-screen player may adopt the elements (`lib/lectures/playbackOwner.ts`).
   *  `tabId` is the pane. */
  const tabId = useTabId();
  const { tabId: stripTab } = usePaneTab();
  const onScreen = useTabActive();
  const owner = useSyncExternalStore(subscribePlaybackOwner, playbackOwner);
  const transcriptVisible = usePlayerPrefs((s) => s.transcriptVisible);
  const dockTab = usePlayerPrefs((s) => s.dockTab);
  const layoutPref = usePlayerPrefs((s) => s.layout);
  const mainPref = usePlayerPrefs((s) => s.mainSource);
  const setPrefs = usePlayerPrefs((s) => s.set);
  const handleDockTabChange = useCallback((dockTab: DockTab) => setPrefs({ dockTab }), [setPrefs]);

  const { urls, sources, downloading, dlProgress } = useLectureSources(lecture);

  /** The boxes in the frames the elements are moved into. */
  const mainHostRef = useRef<HTMLDivElement>(null);
  const secondHostRef = useRef<HTMLDivElement>(null);

  /** Echo360 publishes a camera stream for this capture (`docs/sync.md`). */
  const hasSecondSource = lecture.has_source2 === 1;

  // A two-frame layout needs both files; until then it is one screen.
  const layout = urls[1] && urls[2] ? layoutPref : "single";
  /** The stream in the main frame; the pref holds only while it is on disk. */
  const mainSource: SourceNum = urls[mainPref] ? mainPref : urls[2] && !urls[1] ? 2 : 1;
  const otherSource: SourceNum = mainSource === 1 ? 2 : 1;
  const mainSrc = urls[mainSource];

  /** This player may hold the elements; see `mayAdopt`. */
  const adoptable = mayAdopt(owner, tabId, stripTab, lecture.id);
  /** On screen while the other pane's player has the elements: this one shows
   *  a still and touches nothing until the user plays here. */
  const elsewhere = onScreen && !!mainSrc && !adoptable;
  /** What the adoption does once a user action claims playback here. */
  const startRef = useRef<{ at?: number } | null>(null);
  const playHere = (at?: number) => {
    startRef.current = { at };
    claimPlayback(tabId, stripTab, lecture.id);
  };

  // The frames always show both streams, so a pick in the second frame makes
  // the *other* stream the main one.
  const selectSource = (n: SourceNum) => setPrefs({ mainSource: n });
  const selectSecondSource = (n: SourceNum) =>
    setPrefs({ mainSource: n === 1 ? 2 : 1 });

  const handleDownloadSource = (n: SourceNum) => {
    setError(null);
    downloadLecture(lecture, n)
      .then(onRefresh)
      .catch((e) => setError(`Download failed: ${e}`));
  };
  const handleDownloadVideo = () => handleDownloadSource(1);

  /** Which frame has its source pill open, so it stays put while it is. */
  const [openSwitcher, setOpenSwitcher] = useState<SourceNum | null>(null);

  // Read from SQLite, not the `lecture` row: that row is a snapshot a
  // chaptering run outlives.
  const chapterState = useLectureChapters(lecture.id);
  const chapterStarts = useMemo(
    () => chapterState.chapters.map((c) => c.start_seconds),
    [chapterState.chapters],
  );

  // Found on first open; Done (`lib/lectures/playback/`) and Up Next read it.
  const end = useLectureEnd(lecture.id, lecture.transcript_path);
  const endError = end.state?.status === "error" ? (end.state.error ?? "") : null;

  const { cues, toggleTranscript, awaitingCues, recordTranscript } = useLectureTranscript({
    lecture,
    onRefresh,
    setError,
  });

  const player = useMediaPlayer({
    cues,
    resetKey: lecture.id,
    // Echo360's catalogue length, until the file reports its own.
    fallbackDuration: lecture.duration_seconds,
    dockTab,
    // Captions come from the cues: no transcript, nothing to toggle.
    hasCaptions: !!lecture.transcript_path,
    elsewhere,
    onClaim: playHere,
    onToggleDock: toggleTranscript,
  });
  const { attach, currentTime, atRef, duration, seek, following, onScrollAway, onBackToLive } =
    player;

  const pipAspect = useSharedElement({
    lecture,
    tabId,
    stripTab,
    onScreen,
    adoptable,
    mainSrc,
    mainSource,
    otherSource,
    urls,
    layout,
    mainHostRef,
    secondHostRef,
    startRef,
    attach,
    onRefresh,
  });

  const {
    pipStyle,
    split,
    pipDragging,
    splitting,
    startPipMove,
    startPipResize,
    startSplitDrag,
  } = useSourceLayout(player.videoAreaRef, pipAspect);

  // The dock stays mounted so it can slide in and out.
  const showDock = transcriptVisible;
  /** A lecture without a transcript keeps the Transcript tab, for Transcribe. */
  const hasTranscriptTab = cues.length > 0 || !lecture.transcript_path || awaitingCues;
  /** The tab the dock is actually showing — what T shows or hides. */
  const frontTab = tabInFront(dockTab, hasTranscriptTab);

  /** The chapter the playhead is in; its name sits above the scrub bar. */
  const activeChapterIdx = spanAt(chapterStarts, currentTime);
  const activeChapter =
    activeChapterIdx >= 0 ? chapterState.chapters[activeChapterIdx] : null;

  const { chaptersProps, transcribeProps, chatProps } = useDockProps({
    lecture,
    cues,
    chapterState,
    chapterStarts,
    activeChapterIdx,
    atRef,
    duration,
    seek,
    endError,
    onRetryEnd: end.retry,
    awaitingCues,
    recordTranscript,
  });

  const { controlsVisible } = player;
  const { dock, size, resizing, startDockDrag } = player.dock;

  return (
    <MediaPlayer
      player={player}
      canPlay={!!mainSrc}
      previewSrc={mainSrc}
      chapters={chapterStarts}
      endAt={contentEnd(lecture)}
      error={error}
      dockLabel={
        !lecture.transcript_path && dockTab === "transcript"
          ? "Download transcript"
          : `${showDock ? "Hide" : "Show"} ${DOCK_TAB_NOUN[frontTab]} (T)`
      }
      aboveSeek={
        // The playing chapter's name — the one thing the scrub bar cannot say.
        activeChapter && (
          <div className="select-none truncate pb-1 text-[11.5px] font-medium text-white/90">
            {activeChapter.title}
          </div>
        )
      }
      controls={
        <>
          {!lecture.video_path && (
            <ControlButton
              label="Download video"
              onClick={handleDownloadVideo}
              disabled={downloading}
            >
              {downloading ? (
                <CircleNotch size={16} className="animate-spin" />
              ) : (
                <DownloadSimple size={16} />
              )}
            </ControlButton>
          )}

          {hasSecondSource && (
            <LayoutControl
              layout={layout}
              onChange={(l) => setPrefs({ layout: l })}
              second={sources[2]}
              onDownloadSecond={() => handleDownloadSource(2)}
              onOpenChange={player.onPanelOpenChange}
            />
          )}
        </>
      }
      overlay={
        <>
          {elsewhere && (
            <ElsewhereOverlay
              title={lecture.title}
              playing={owner.playing}
              onPlayHere={() => playHere()}
            />
          )}
          <UpNextPanel
            lecture={lecture}
            element={player.element}
            currentTime={currentTime}
            duration={duration}
            elsewhere={elsewhere}
            controlsVisible={controlsVisible}
          />
        </>
      }
      dock={
        <TranscriptPanel
          cues={cues}
          activeCueIdx={player.activeCueIdx}
          tab={dockTab}
          onTabChange={handleDockTabChange}
          chapters={chaptersProps}
          chat={chatProps}
          transcribe={transcribeProps}
          dock={dock}
          size={size}
          open={showDock}
          resizing={resizing}
          onSeek={seek}
          onClose={player.closeDock}
          onHeaderPointerDown={startDockDrag}
          following={following}
          onScrollAway={onScrollAway}
          onBackToLive={onBackToLive}
        />
      }
    >
      {mainSrc ? (
        <>
          {/* `flexGrow` on a zero basis, so the split divides what is left
              after the divider. */}
          <VideoFrame
            hostRef={mainHostRef}
            source={mainSource}
            sources={sources}
            showSwitcher={hasSecondSource}
            chromeVisible={controlsVisible}
            pinned={openSwitcher === mainSource}
            onSelectSource={selectSource}
            onDownloadSource={handleDownloadSource}
            onSwitcherOpenChange={(open) =>
              setOpenSwitcher(open ? mainSource : null)
            }
            className={layout === "stack" ? undefined : "flex-1"}
            style={
              layout === "stack" ? { flexGrow: split, flexBasis: 0 } : undefined
            }
          />

          {layout === "stack" && (
            <>
              <StackDivider onPointerDown={startSplitDrag} dragging={splitting} />
              <VideoFrame
                hostRef={secondHostRef}
                source={otherSource}
                sources={sources}
                showSwitcher={hasSecondSource}
                chromeVisible={controlsVisible}
                pinned={openSwitcher === otherSource}
                onSelectSource={selectSecondSource}
                onDownloadSource={handleDownloadSource}
                onSwitcherOpenChange={(open) =>
                  setOpenSwitcher(open ? otherSource : null)
                }
                style={{ flexGrow: 1 - split, flexBasis: 0 }}
              />
            </>
          )}

          {layout === "pip" && (
            <VideoFrame
              hostRef={secondHostRef}
              source={otherSource}
              sources={sources}
              showSwitcher={hasSecondSource}
              chromeVisible={controlsVisible}
              pinned={openSwitcher === otherSource}
              onSelectSource={selectSecondSource}
              onDownloadSource={handleDownloadSource}
              onSwitcherOpenChange={(open) =>
                setOpenSwitcher(open ? otherSource : null)
              }
              // Above the main picture, under the control bar (z-30).
              className={cn(
                "absolute z-20 touch-none rounded-lg border border-white/20 bg-black",
                "shadow-2xl shadow-black/60",
                pipDragging ? "cursor-grabbing" : "cursor-grab",
              )}
              style={pipStyle}
              onPointerDown={startPipMove}
            >
              {/* Hidden with the bar like the switcher; pinned while dragging. */}
              {PIP_CORNERS.map((corner) => (
                <span
                  key={corner}
                  aria-hidden="true"
                  onPointerDown={(e) => startPipResize(e, corner)}
                  className={cn(
                    "absolute z-10 size-5 touch-none transition-opacity will-change-[opacity]",
                    pipDragging
                      ? "opacity-100"
                      : controlsVisible
                        ? "opacity-0 group-hover/frame:opacity-100"
                        : "pointer-events-none opacity-0",
                    CORNER_STYLE[corner],
                  )}
                >
                  <span className="absolute inset-1 rounded-[3px] bg-white/70" />
                </span>
              ))}
            </VideoFrame>
          )}
        </>
      ) : (
        <DownloadPrompt
          lecture={lecture}
          downloading={downloading}
          progress={dlProgress}
          onDownload={handleDownloadVideo}
        />
      )}
    </MediaPlayer>
  );
}
