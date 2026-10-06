import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { CircleNotch, DownloadSimple, Play } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { useNavigate } from "react-router-dom";
import { useWindowEvent } from "@/hooks/useEvents";
import { cn } from "@/lib/utils";
import {
  dlKey,
  downloadLecture,
  isDownloading,
  LECTURE_DOWNLOADED_EVENT,
  useLectureDownloads,
} from "@/stores/lectureDownloadStore";
import { Button } from "@/components/ui/button";
import { mediaSrc, parseVtt, spanAt, fmtClockSecs, type Cue } from "@/lib/media";
import {
  updateLectureTranscriptPath,
  videoPathFor,
  type Lecture,
  type SourceNum,
} from "@/lib/db";
import {
  LECTURE_PROGRESS_EVENT,
  completeLecture,
  parkLectureVideos,
  playOnAdopt,
  playbackClaim,
  releaseLecturePlayer,
  syncLectureSources,
  videoForSource,
  type SourcePlan,
} from "@/lib/lecturePlayback";
import { contentEnd, isWatched, upNextFrom } from "@/lib/lectureEnd";
import {
  claimPlayback,
  mayAdopt,
  playbackOwner,
  subscribePlaybackOwner,
} from "@/lib/playbackOwner";
import { usePaneTab, useTabActive, useTabId } from "@/components/tabs/TabContext";
import {
  LECTURES_CHANGED_EVENT,
  fmtDurationSecs,
  fmtLectureDate,
  lectureGrabFrames,
  lecturePagePath,
} from "@/lib/lectures";
import { useLectureChapters } from "@/hooks/useLectureChapters";
import { useLectureEnd } from "@/hooks/useLectureEnd";
import { useLectureReading } from "@/hooks/useLectureReading";
import {
  TRANSCRIBED_EVENT,
  settleTranscription,
  useTranscription,
  type TranscribedDetail,
} from "@/hooks/useTranscription";
import type { ChaptersPanelProps } from "@/components/lectures/ChaptersPanel";
import type { ReadingListProps } from "@/components/lectures/ReadingList";
import type { LectureChatPanelProps } from "@/components/lectures/LectureChatPanel";
import type { TranscribeEmptyProps } from "@/components/media/TranscribeEmpty";
import { PIP_CORNERS, useSourceLayout, type PipCorner } from "@/hooks/useSourceLayout";
import { usePlayerPrefs, type DockTab } from "@/stores/playerPrefsStore";
import {
  LayoutControl,
  SOURCES,
  SOURCE_HINT,
  SourceSwitcher,
  type SourceState,
  type SourceStates,
} from "@/components/lectures/SourceControls";
import { ControlButton, MediaPlayer, useMediaPlayer } from "@/components/media/MediaPlayer";
import { TranscriptPanel, tabInFront } from "@/components/lectures/TranscriptPanel";
import { UpNextCard, useEnded, useNextLecture } from "@/components/lectures/UpNext";

/** Seconds of transcript before the playhead that a chat moment carries. */
const MOMENT_TRANSCRIPT_S = 60;

/** What the dock button calls the tab in front. */
const DOCK_TAB_NOUN: Record<DockTab, string> = {
  chapters: "chapters",
  transcript: "transcript",
  chat: "chat",
};

/** One picture: the box a long-lived `<video>` is moved into
 *  (`lib/lecturePlayback.ts`), plus its source switcher. */
function VideoFrame({
  hostRef,
  source,
  sources,
  showSwitcher,
  chromeVisible,
  pinned,
  onSelectSource,
  onDownloadSource,
  onSwitcherOpenChange,
  onPointerDown,
  className,
  style,
  children,
}: {
  hostRef: React.RefObject<HTMLDivElement | null>;
  source: SourceNum;
  sources: SourceStates;
  /** False for a single-stream capture: nothing to switch to. */
  showSwitcher: boolean;
  /** The control bar is showing, so frame chrome may show too. */
  chromeVisible: boolean;
  /** This frame's switcher is open, so it stays put whatever the pointer does. */
  pinned: boolean;
  onSelectSource: (source: SourceNum) => void;
  onDownloadSource: (source: SourceNum) => void;
  onSwitcherOpenChange: (open: boolean) => void;
  /** Set on the PIP, where the whole box is the drag handle. */
  onPointerDown?: (e: React.PointerEvent) => void;
  className?: string;
  style?: React.CSSProperties;
  children?: React.ReactNode;
}) {
  return (
    <div
      className={cn("group/frame relative min-h-0 min-w-0 overflow-hidden", className)}
      style={style}
      onPointerDown={onPointerDown}
    >
      {/* Empty on purpose: the shared element is moved in here. */}
      <div ref={hostRef} className="h-full w-full" />

      {showSwitcher && (
        <div
          className={cn(
            "absolute left-2 top-2 z-20 transition-opacity will-change-[opacity] duration-150",
            pinned
              ? "opacity-100"
              : chromeVisible
                ? "opacity-0 group-hover/frame:opacity-100"
                : "pointer-events-none opacity-0",
          )}
          // Inside the PIP the whole box is a drag handle; the pill is not.
          onPointerDown={(e) => e.stopPropagation()}
        >
          <SourceSwitcher
            active={source}
            states={sources}
            onSelect={onSelectSource}
            onDownload={onDownloadSource}
            onOpenChange={onSwitcherOpenChange}
          />
        </div>
      )}

      {children}
    </div>
  );
}

/** Where a corner handle sits, and which way it resizes from there. */
const CORNER_STYLE: Record<PipCorner, string> = {
  nw: "left-0 top-0 cursor-nwse-resize",
  ne: "right-0 top-0 cursor-nesw-resize",
  sw: "bottom-0 left-0 cursor-nesw-resize",
  se: "bottom-0 right-0 cursor-nwse-resize",
};

/** The divider between the two stacked screens — drag it to change the split. */
function StackDivider({
  onPointerDown,
  dragging,
}: {
  onPointerDown: (e: React.PointerEvent) => void;
  dragging: boolean;
}) {
  return (
    <div
      role="separator"
      aria-orientation="horizontal"
      aria-label="Resize screens"
      onPointerDown={onPointerDown}
      className={cn(
        "group/split relative z-20 h-1.5 shrink-0 cursor-row-resize touch-none",
        "transition-colors",
        dragging ? "bg-brand" : "bg-white/10 hover:bg-white/30",
      )}
    >
      <span
        className={cn(
          "pointer-events-none absolute left-1/2 top-1/2 h-[2px] w-8 -translate-x-1/2 -translate-y-1/2",
          "rounded-full bg-white/50 opacity-0 transition-opacity will-change-[opacity] group-hover/split:opacity-100",
        )}
      />
    </div>
  );
}

interface LecturePlayerProps {
  lecture: Lecture;
  /** Fired after anything persisted changes (progress, downloads). */
  onRefresh: () => void;
}

/**
 * The lecture page's player: `MediaPlayer` over the shared elements
 * (`lib/lecturePlayback.ts`), with what only a lecture has — two sources, the
 * pane that owns playback, chapters, the reading copy, chat and downloads.
 */
export function LecturePlayer({ lecture, onRefresh }: LecturePlayerProps) {
  const [cues, setCues] = useState<Cue[]>([]);
  const [error, setError] = useState<string | null>(null);

  /** Every tab stays mounted and there is one `<video>` per source app-wide, so
   *  only an on-screen player may adopt the elements (`lib/playbackOwner.ts`).
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

  // Global: a download outlives this component (it runs in Rust).
  const downloading = useLectureDownloads((s) => isDownloading(s, lecture.id));
  const downloadingSecond = useLectureDownloads((s) => isDownloading(s, lecture.id, 2));
  const dlProgress = useLectureDownloads((s) => s.progress[dlKey(lecture.id, 1)] ?? null);
  const secondDlProgress = useLectureDownloads((s) => s.progress[dlKey(lecture.id, 2)] ?? null);

  /** The boxes in the frames the elements are moved into. */
  const mainHostRef = useRef<HTMLDivElement>(null);
  const secondHostRef = useRef<HTMLDivElement>(null);

  // ── Sources ──────────────────────────────────────────────────────────────

  // Served over localhost HTTP, not convertFileSrc — WebKit's media stack
  // refuses custom-scheme (asset://) sources outright. See lib/media.ts.
  // Tagged with their lecture: when the route moves on to another one, the
  // last lecture's URLs must not be handed to the new one while these resolve.
  const [resolved, setResolved] = useState<{ id: string } & Record<SourceNum, string | null>>({
    id: lecture.id,
    1: null,
    2: null,
  });
  useEffect(() => {
    let stale = false;
    const id = lecture.id;
    Promise.all(
      SOURCES.map(async (n) => {
        const path = videoPathFor(lecture, n);
        return [n, path ? await mediaSrc(path) : null] as const;
      }),
    ).then((pairs) => {
      if (!stale) {
        setResolved({ id, 1: pairs[0][1], 2: pairs[1][1] });
      }
    });
    return () => {
      stale = true;
    };
  }, [lecture.id, lecture.video_path, lecture.video2_path]); // eslint-disable-line react-hooks/exhaustive-deps
  const urls: Record<SourceNum, string | null> =
    resolved.id === lecture.id ? resolved : { 1: null, 2: null };

  /** Downloaded / downloading, per source, for the two source controls. */
  const sources: SourceStates = useMemo(() => {
    const of = (n: SourceNum): SourceState => {
      const p = n === 1 ? dlProgress : secondDlProgress;
      return {
        ready: !!videoPathFor(lecture, n),
        busy: n === 1 ? downloading : downloadingSecond,
        percent: p?.percent ?? 0,
        phase: p?.phase ?? "",
      };
    };
    return { 1: of(1), 2: of(2) };
  }, [dlProgress, secondDlProgress, downloading, downloadingSecond, lecture.video_path, lecture.video2_path]);

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

  /** The inset picture's own aspect, read in the reconcile effect below. */
  const [pipAspect, setPipAspect] = useState(16 / 9);

  /** Which frame has its source pill open, so it stays put while it is. */
  const [openSwitcher, setOpenSwitcher] = useState<SourceNum | null>(null);

  // ── Chapters ─────────────────────────────────────────────────────────────

  // Read from SQLite, not the `lecture` row: that row is a snapshot a
  // chaptering run outlives.
  const chapterState = useLectureChapters(lecture.id);
  const chapterStarts = useMemo(
    () => chapterState.chapters.map((c) => c.start_seconds),
    [chapterState.chapters],
  );

  // ── Where the lecture ends ───────────────────────────────────────────────

  // Found on first open; Done (`lib/lecturePlayback.ts`) and Up Next read it.
  const end = useLectureEnd(lecture.id, lecture.transcript_path);
  const endError = end.state?.status === "error" ? (end.state.error ?? "") : null;

  // ── Reading copy ─────────────────────────────────────────────────────────

  // Independent of chapters — a lecture can have either, both or neither.
  const readingState = useLectureReading(lecture.id);
  const lineStarts = useMemo(
    () => readingState.lines.map((l) => l.start_seconds),
    [readingState.lines],
  );

  // ── Transcript loading ───────────────────────────────────────────────────

  const loadTranscript = useCallback(async (path: string) => {
    try {
      const vtt = await invoke<string>("echo360_read_transcript", { path });
      setCues(parseVtt(vtt));
    } catch {
      /* transcript unreadable */
    }
  }, []);

  // ── Downloads ────────────────────────────────────────────────────────────

  // Downloads the transcript alongside the video; both land in the DB before
  // it resolves, so the refresh picks the paths up.
  const handleDownloadVideo = () => handleDownloadSource(1);

  const handleDownloadTranscript = async () => {
    setError(null);
    try {
      const path = await invoke<string>("echo360_download_transcript", {
        lessonId: lecture.lesson_id,
        mediaId: lecture.id,
        canvasCourseId: lecture.subject_id,
      });
      await updateLectureTranscriptPath(lecture.id, path);
      onRefresh();
      await loadTranscript(path);
      setPrefs({ transcriptVisible: true });
    } catch (e) {
      setError(`Transcript download failed: ${e}`);
      // Echo360 has none: the Transcript tab offers Transcribe instead.
      setPrefs({ transcriptVisible: true, dockTab: "transcript" });
    }
  };

  // T does what the dock button does: parse a transcript on disk but not loaded,
  // fetch one if the *preference* is the transcript tab (it outlives
  // `tabInFront` dropping that tab), otherwise show or hide the dock.
  const toggleTranscript = () => {
    if (lecture.transcript_path && cues.length === 0) {
      loadTranscript(lecture.transcript_path);
    } else if (!lecture.transcript_path && dockTab === "transcript") {
      handleDownloadTranscript();
    } else {
      setPrefs({ transcriptVisible: !transcriptVisible });
    }
  };

  // ── The player ───────────────────────────────────────────────────────────

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

  // Reset per lecture: fresh transcript; the player resets its own clock.
  useEffect(() => {
    setCues([]);
    setError(null);

    if (lecture.transcript_path) {
      // A finished run waits, as a spinner, until its cues are read.
      const video = lecture.video_path;
      loadTranscript(lecture.transcript_path).finally(
        () => video && settleTranscription(video),
      );
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lecture.id, lecture.transcript_path, lecture.video_path]);

  // ── Transcription ────────────────────────────────────────────────────────

  /** Records a finished run's VTT as the transcript, as a download does;
   *  it runs even if this player has unmounted by then. */
  const lectureId = lecture.id;
  const recordTranscript = useCallback(
    async (vtt: string) => {
      await updateLectureTranscriptPath(lectureId, vtt);
      window.dispatchEvent(new CustomEvent(LECTURE_DOWNLOADED_EVENT, { detail: lectureId }));
    },
    [lectureId],
  );

  /** A finished run keeps the Transcript tab on its spinner until the cues
   *  load, so the tab neither drops nor offers a second run. */
  const { run: transcribeRun } = useTranscription(lecture.video_path);
  const awaitingCues = transcribeRun?.phase === "done" && cues.length === 0;

  useWindowEvent(TRANSCRIBED_EVENT, (e) => {
    const { path } = (e as CustomEvent<TranscribedDetail>).detail;
    if (path === lecture.video_path) onRefresh();
  });

  // ── The shared element ───────────────────────────────────────────────────

  useEffect(() => {
    const mainHost = mainHostRef.current;
    // Behind another tab, or beside the player that has them, the elements
    // stay where they are; this player adopts them once `mayAdopt` says so.
    // Re-checked against the live owner: two players can both pass at render.
    if (
      !mainHost ||
      !mainSrc ||
      !onScreen ||
      !adoptable ||
      !mayAdopt(playbackOwner(), tabId, stripTab, lecture.id)
    ) {
      attach(null);
      return;
    }

    // The main frame is first, and first is the leader: its audio is what plays.
    const plan: SourcePlan[] = [{ source: mainSource, src: mainSrc, host: mainHost }];
    const secondHost = secondHostRef.current;
    const secondSrc = urls[otherSource];
    if (layout !== "single" && secondHost && secondSrc) {
      plan.push({ source: otherSource, src: secondSrc, host: secondHost });
    }

    const start = startRef.current ?? undefined;
    startRef.current = null;
    const v = syncLectureSources(lecture, plan, tabId, stripTab, start);
    // Parking restores the elements only if still ours — another player (e.g.
    // expand-to-tab) may have taken them since.
    const claim = playbackClaim();
    if (!v) return;
    const detach = attach(v);

    // Read here: the inset element exists only once the plan is reconciled.
    const inset = plan.length > 1 ? videoForSource(plan[1].source) : null;
    const readAspect = () => {
      if (inset?.videoWidth && inset.videoHeight) {
        setPipAspect(inset.videoWidth / inset.videoHeight);
      }
    };
    readAspect();
    inset?.addEventListener("loadedmetadata", readAspect);

    return () => {
      detach();
      inset?.removeEventListener("loadedmetadata", readAspect);
      // Unmounting is a tab switch, not a stop: the elements go back to their
      // off-screen host and carry on. Closing the lecture is what stops them.
      parkLectureVideos(claim);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lecture.id, urls[1], urls[2], layout, mainSource, onScreen, adoptable, tabId, stripTab]);

  // Unmounting paused gives playback up, so the other pane's player can take it.
  useEffect(() => () => releaseLecturePlayer(tabId), [tabId]);

  // Progress is saved by the module, even while nothing is mounted.
  useWindowEvent(LECTURE_PROGRESS_EVENT, () => onRefresh());

  const {
    pipStyle,
    split,
    pipDragging,
    splitting,
    startPipMove,
    startPipResize,
    startSplitDrag,
  } = useSourceLayout(player.videoAreaRef, pipAspect);

  // ── Render ───────────────────────────────────────────────────────────────

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

  /** The reading line the playhead is in. */
  const activeLineIdx = spanAt(lineStarts, currentTime);

  // One memoised bag per tab: `TranscriptPanel` is memo'd against a player that
  // re-renders on every `timeupdate`.
  const chaptersProps: ChaptersPanelProps = useMemo(
    () => ({
      chapters: chapterState.chapters,
      activeIdx: activeChapterIdx,
      atRef,
      duration,
      status: chapterState.status,
      error: chapterState.error,
      since: chapterState.since,
      progress: chapterState.progress,
      busy: chapterState.busy,
      downloaded: !!lecture.video_path,
      onSeek: seek,
      onFind: chapterState.find,
      endError,
      onRetryEnd: end.retry,
    }),
    [
      chapterState.chapters,
      chapterState.status,
      chapterState.error,
      chapterState.since,
      chapterState.progress,
      chapterState.busy,
      chapterState.find,
      activeChapterIdx,
      atRef,
      duration,
      lecture.video_path,
      seek,
      endError,
      end.retry,
    ],
  );

  /** The Transcript tab's Enhanced register. That tab only exists when there
   *  are cues, so the bag carries no `hasTranscript`. */
  const readingProps: Omit<ReadingListProps, "picker"> = useMemo(
    () => ({
      lines: readingState.lines,
      activeLineIdx,
      status: readingState.status,
      error: readingState.error,
      since: readingState.since,
      progress: readingState.progress,
      busy: readingState.busy,
      onWrite: readingState.write,
      downloaded: !!lecture.video_path,
      open: showDock,
      active: frontTab === "transcript",
      following,
      onScrollAway,
      onBackToLive,
      onSeek: seek,
    }),
    [
      readingState.lines,
      readingState.status,
      readingState.error,
      readingState.since,
      readingState.progress,
      readingState.busy,
      readingState.write,
      activeLineIdx,
      lecture.video_path,
      showDock,
      frontTab,
      following,
      onScrollAway,
      onBackToLive,
      seek,
    ],
  );

  /** The Transcript tab while the lecture has no transcript: Transcribe runs
   *  on the main recording. */
  const transcribeProps: TranscribeEmptyProps | null = useMemo(
    () =>
      lecture.transcript_path && !awaitingCues
        ? null
        : { path: lecture.video_path, after: recordTranscript },
    [lecture.transcript_path, awaitingCues, lecture.video_path, recordTranscript],
  );

  /**
   * The moment a dock message carries, as `SendOptions.context`
   * (`docs/harness.md`): chapter, the last minute of cues inline, and a frame of
   * *every* downloaded stream — the teaching may be on either source. A frame
   * that cannot be grabbed is dropped, never the message.
   */
  const buildMoment = useCallback(
    async (at: number): Promise<string> => {
      const parts: string[] = [
        `The student is at ${fmtClockSecs(at, true)} of this recording (second ${at}).`,
      ];

      const idx = spanAt(chapterStarts, at);
      const chapter = idx >= 0 ? chapterState.chapters[idx] : null;
      if (chapter) {
        parts.push(
          `That is inside chapter ${idx + 1}, "${chapter.title}", which starts at ${fmtClockSecs(chapter.start_seconds, true)}.`,
        );
      }

      const from = Math.max(0, at - MOMENT_TRANSCRIPT_S);
      const said = cues
        .filter((c) => c.start >= from && c.start <= at)
        .map((c) => c.text)
        .join(" ")
        .trim();
      if (said) {
        parts.push(
          `What was said between ${fmtClockSecs(from, true)} and ${fmtClockSecs(at, true)}:\n\n${said}`,
        );
      }

      const frames = await lectureGrabFrames(lecture.id, at).catch(() => []);
      if (frames.length) {
        const shots = frames
          .map((f) => `- source ${f.source} (usually the ${SOURCE_HINT[f.source].toLowerCase()}): \`${f.path}\``)
          .join("\n");
        parts.push(
          frames.length > 1
            ? `Frames of every stream of this capture at that second. Echo360 numbers the streams rather than naming them and either may be the one being taught from — a board is often only on the camera, slides only on the screen capture — so read all of them:\n\n${shots}`
            : `A frame of the recording at that second:\n\n${shots}`,
        );
      }

      return parts.join("\n\n");
    },
    [lecture.id, cues, chapterStarts, chapterState.chapters],
  );

  // Stable across `timeupdate`: the playhead travels by ref.
  const chatProps: LectureChatPanelProps = useMemo(
    () => ({ lectureId: lecture.id, atRef, buildMoment }),
    [lecture.id, atRef, buildMoment],
  );

  const { controlsVisible } = player;
  const { dock, size, resizing, startDockDrag } = player.dock;

  // ── Up Next ──────────────────────────────────────────────────────────────

  const navigate = useNavigate();
  const next = useNextLecture(lecture);
  const ended = useEnded(player.element, lecture.id);
  /** × hides the card until the lecture is opened again. */
  const [dismissed, setDismissed] = useState<string | null>(null);
  const upNextAt = upNextFrom(lecture, duration);
  const showUpNext =
    !!next &&
    !!player.element &&
    !elsewhere &&
    dismissed !== lecture.id &&
    upNextAt != null &&
    currentTime >= upNextAt;

  /** Play: this lecture is Done, and this tab moves on to the next, playing. */
  const leavingRef = useRef(false);
  const playNext = async () => {
    if (!next || leavingRef.current) return;
    leavingRef.current = true;
    try {
      await completeLecture(lecture.id);
      window.dispatchEvent(new CustomEvent(LECTURES_CHANGED_EVENT));
      // One already watched starts over rather than at its end.
      const over = !!next.completed || isWatched(next, next.progress_seconds, 0);
      playOnAdopt(next.id, over ? 0 : undefined);
      navigate(lecturePagePath(next), { replace: true });
    } finally {
      leavingRef.current = false;
    }
  };

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
            // Over the frames, not instead of them: their hosts stay mounted
            // for the moment the elements come back.
            <div className="absolute inset-0 z-40 flex flex-col items-center justify-center gap-4 bg-black p-6 text-white/60">
              <p className="max-w-full truncate text-sm font-medium text-white">{lecture.title}</p>
              <p className="text-xs">
                {owner.playing ? "Playing in the other pane" : "Paused in the other pane"}
              </p>
              <Button
                size="sm"
                className="gap-2 bg-white/10 hover:bg-white/20 hover:text-white text-white border-white/20"
                variant="outline"
                onClick={() => playHere()}
              >
                <Play size={14} weight="fill" /> Play here
              </Button>
            </div>
          )}
          {showUpNext && next && (
            <UpNextCard
              key={next.id}
              next={next}
              ended={ended}
              liftedAbove={controlsVisible}
              onPlay={playNext}
              onDismiss={() => setDismissed(lecture.id)}
            />
          )}
        </>
      }
      dock={
        <TranscriptPanel
          cues={cues}
          activeCueIdx={player.activeCueIdx}
          tab={dockTab}
          onTabChange={handleDockTabChange}
          chapters={chaptersProps}
          reading={readingProps}
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
        <div className="flex-1 flex flex-col items-center justify-center gap-4 text-white/60 p-6">
          <p className="text-sm font-medium text-white">{lecture.title}</p>
          <p className="text-xs">
            {fmtLectureDate(lecture.date)} · {fmtDurationSecs(lecture.duration_seconds)}
          </p>
          {downloading ? (
            <div className="flex items-center gap-2 text-sm">
              <CircleNotch size={16} className="animate-spin" />
              <span>
                {dlProgress?.phase === "trimming"
                  ? "Trimming…"
                  : `Downloading… ${dlProgress?.percent ?? 0}%`}
              </span>
            </div>
          ) : (
            <Button
              size="sm"
              className="gap-2 bg-white/10 hover:bg-white/20 text-white border-white/20"
              variant="outline"
              onClick={handleDownloadVideo}
            >
              <DownloadSimple size={14} /> Download video
            </Button>
          )}
        </div>
      )}
    </MediaPlayer>
  );
}
