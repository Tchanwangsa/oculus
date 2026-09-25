import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowsIn,
  ArrowsOut,
  CircleNotch,
  ClosedCaptioning,
  DownloadSimple,
  Pause,
  Play,
  SidebarSimple,
} from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useWindowEvent } from "@/hooks/useEvents";
import { cn } from "@/lib/utils";
import {
  dlKey,
  downloadLecture,
  isDownloading,
  useLectureDownloads,
} from "@/stores/lectureDownloadStore";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { mediaSrc } from "@/lib/media";
import {
  updateLectureTranscriptPath,
  videoPathFor,
  type Lecture,
  type SourceNum,
} from "@/lib/db";
import {
  LECTURE_PROGRESS_EVENT,
  parkLectureVideos,
  playbackClaim,
  syncLectureSources,
  videoForSource,
  type PlaybackHost,
  type SourcePlan,
} from "@/lib/lecturePlayback";
import { useTabActive, useTabId } from "@/components/tabs/TabContext";
import {
  parseVtt,
  spanAt,
  fmtDurationSecs,
  fmtClockSecs,
  fmtLectureDate,
  lectureGrabFrames,
  type Cue,
} from "@/lib/lectures";
import { useLectureChapters } from "@/hooks/useLectureChapters";
import { useLectureReading } from "@/hooks/useLectureReading";
import type { ChaptersPanelProps } from "@/components/lectures/ChaptersPanel";
import type { ReadingListProps } from "@/components/lectures/ReadingList";
import type { LectureChatPanelProps } from "@/components/lectures/LectureChatPanel";
import { useTranscriptDock, type Dock } from "@/hooks/useTranscriptDock";
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
import { CaptionOverlay } from "@/components/lectures/CaptionOverlay";
import { ScrubPreview } from "@/components/lectures/ScrubPreview";
import { SpeedControl } from "@/components/lectures/SpeedControl";
import { VolumeControl } from "@/components/lectures/VolumeControl";
import {
  DockDropPreview,
  DockResizeHandle,
  TranscriptPanel,
  tabInFront,
} from "@/components/lectures/TranscriptPanel";

/** Seconds of transcript before the playhead that a chat moment carries. */
const MOMENT_TRANSCRIPT_S = 60;

/** Turns `SidebarSimple` to face the dock's edge; the glyph's divider is on
 *  the left, so left is unrotated. */
const DOCK_ICON_FACING: Record<Dock, string> = {
  left: "",
  right: "rotate-180",
  top: "rotate-90",
  bottom: "-rotate-90",
};

/** What the dock button calls the tab in front. */
const DOCK_TAB_NOUN: Record<DockTab, string> = {
  chapters: "chapters",
  transcript: "transcript",
  chat: "chat",
};

/** Is a popover open? Its dismissing click must not also play/pause or seek.
 *  Radix dismisses on `click` at `document`, so during the video's handler the
 *  panel is still `data-state=open`. */
const panelOnScreen = () =>
  !!document.querySelector('[data-slot="popover-content"][data-state="open"]');

/** Video scrub bar. `offsetX / offsetWidth` keeps the maths in the element's
 *  own coordinates. Colours are fixed: it sits on the video scrim. */
function SeekBar({
  value,
  max,
  previewSrc,
  chapters,
  onSeek,
}: {
  value: number;
  max: number;
  /** Source for the hover thumbnail; `null` until the video is downloaded. */
  previewSrc: string | null;
  /** Chapter boundaries, in seconds — notched into the track. */
  chapters: number[];
  onSeek: (seconds: number) => void;
}) {
  const pct = Math.min(100, Math.max(0, (value / max) * 100));

  // `armed` mounts the preview decoder on first hover; the position is kept
  // while it fades out.
  const [armed, setArmed] = useState(false);
  const [hovering, setHovering] = useState(false);
  const [preview, setPreview] = useState({ x: 0, t: 0, w: 1 });
  /** Pointer inside the bar — a drag that ends outside it should not stick. */
  const insideRef = useRef(false);

  const trackFromEvent = (e: React.PointerEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    const x = e.nativeEvent.offsetX;
    const frac = Math.min(1, Math.max(0, x / el.offsetWidth));
    setPreview({ x, t: frac * max, w: el.offsetWidth });
    return frac * max;
  };

  return (
    <div
      role="slider"
      aria-label="Seek"
      aria-valuemin={0}
      aria-valuemax={Math.floor(max)}
      aria-valuenow={Math.floor(value)}
      className="group/seek relative flex w-full items-center h-3.5 cursor-pointer touch-none select-none"
      onPointerEnter={() => {
        insideRef.current = true;
        setArmed(true);
        setHovering(true);
      }}
      onPointerLeave={(e) => {
        insideRef.current = false;
        // Mid-drag the pointer is still ours; the preview follows it out.
        if (!e.currentTarget.hasPointerCapture(e.pointerId)) setHovering(false);
      }}
      onLostPointerCapture={() => {
        if (!insideRef.current) setHovering(false);
      }}
      onPointerDown={(e) => {
        // The click that closes a panel is not also a seek.
        if (panelOnScreen()) return;
        e.currentTarget.setPointerCapture(e.pointerId);
        setHovering(true);
        onSeek(trackFromEvent(e));
      }}
      onPointerMove={(e) => {
        const t = trackFromEvent(e);
        if (e.currentTarget.hasPointerCapture(e.pointerId)) onSeek(t);
      }}
    >
      {armed && (
        <ScrubPreview
          src={previewSrc}
          time={preview.t}
          x={preview.x}
          trackWidth={preview.w}
          forceHours={max >= 3600}
          visible={hovering}
        />
      )}

      {/* pointer-events-none children keep `offsetX` relative to the root */}
      <div
        className={cn(
          "pointer-events-none relative w-full overflow-hidden rounded-full bg-white/25",
          "h-[3px] transition-[height] group-hover/seek:h-[5px]",
        )}
      >
        <div className="absolute h-full bg-brand" style={{ width: `${pct}%` }} />
        {/* Notches, not segments, so the track keeps its rounded ends and hover
            growth. The boundary at second 0 is the left edge: not drawn. */}
        {chapters.map((t) =>
          t > 0 && t < max ? (
            <span
              key={t}
              aria-hidden
              className="absolute inset-y-0 w-[2px] -translate-x-1/2 bg-black/55"
              style={{ left: `${(t / max) * 100}%` }}
            />
          ) : null,
        )}
      </div>
      <div
        className={cn(
          "pointer-events-none absolute size-3 -translate-x-1/2 rounded-full bg-brand shadow-sm",
          "transition-transform group-hover/seek:scale-115",
        )}
        style={{ left: `${pct}%` }}
      />
    </div>
  );
}

/** A control-bar button. Not shadcn `Button`: its ghost hover (`accent`)
 *  vanishes on video. */
function ControlButton({
  label,
  onClick,
  disabled,
  active,
  children,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  active?: boolean;
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          onClick={onClick}
          disabled={disabled}
          aria-label={label}
          className={cn(
            "relative inline-flex size-8 shrink-0 items-center justify-center rounded-full",
            "transition-colors hover:bg-white/15 hover:text-white",
            "disabled:pointer-events-none disabled:opacity-40",
            active ? "text-white" : "text-white/85",
          )}
        >
          {children}
          {/* An "on" mark: on a frame a lit icon reads the same as an unlit one. */}
          {active && (
            <span className="absolute bottom-[3px] h-[2px] w-3.5 rounded-full bg-white" />
          )}
        </button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

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
            "absolute left-2 top-2 z-20 transition-opacity duration-150",
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
          "rounded-full bg-white/50 opacity-0 transition-opacity group-hover/split:opacity-100",
        )}
      />
    </div>
  );
}

interface LecturePlayerProps {
  lecture: Lecture;
  /** Fired after anything persisted changes (progress, downloads). */
  onRefresh: () => void;
  /** Off in the peek panel: fullscreen is the window's, and a peek is a panel
   *  over a page. Expand promotes it to a tab first. */
  allowFullscreen?: boolean;
  /** Off in the side panel, which is too small for a readable dock: no button,
   *  no T key. The preference is untouched for the page route. */
  allowDock?: boolean;
  /** Which player this is. Navigating a tab strands the page's player but not
   *  the peek; `lib/lecturePlayback.ts` holds it, `lib/tabRouters.ts` asks. */
  host?: PlaybackHost;
}

/** The whole lecture player — video, controls, dock — for the peek and the page. */
export function LecturePlayer({
  lecture,
  onRefresh,
  allowFullscreen = true,
  allowDock = true,
  host = "page",
}: LecturePlayerProps) {
  const [cues, setCues] = useState<Cue[]>([]);
  const [activeCueIdx, setActiveCueIdx] = useState(-1);
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  /** The playhead for consumers that must not re-render with it: a prop would
   *  break `TranscriptPanel`'s memo on every `timeupdate`. */
  const atRef = useRef(0);
  /** The file's own length — Echo360's catalogue duration can run short of the
   *  recording. `lecture.duration_seconds` stands in until metadata arrives. */
  const [fileDuration, setFileDuration] = useState(0);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** The dock's list is tracking playback. Shared by Transcript and Read, which
   *  are never mounted together. */
  const [following, setFollowing] = useState(true);
  /** Controls are over the frame, so they fade out of the way while playing. */
  const [controlsVisible, setControlsVisible] = useState(true);

  const speed = usePlayerPrefs((s) => s.speed);
  /** Every tab stays mounted and there is one `<video>` per source app-wide, so
   *  only the on-screen player answers keys or claims the elements. */
  const tabId = useTabId();
  const onScreen = useTabActive();
  const volume = usePlayerPrefs((s) => s.volume);
  const muted = usePlayerPrefs((s) => s.muted);
  const captionsEnabled = usePlayerPrefs((s) => s.captionsEnabled);
  const transcriptVisible = usePlayerPrefs((s) => s.transcriptVisible);
  const dockTab = usePlayerPrefs((s) => s.dockTab);
  const layoutPref = usePlayerPrefs((s) => s.layout);
  const mainPref = usePlayerPrefs((s) => s.mainSource);
  const setPrefs = usePlayerPrefs((s) => s.set);

  // Global: a download outlives this component (it runs in Rust).
  const downloads = useLectureDownloads();
  const downloading = isDownloading(downloads, lecture.id);
  const dlProgress = downloads.progress[dlKey(lecture.id, 1)] ?? null;

  /** The leader element while this player has it; see lib/lecturePlayback.ts. */
  const videoRef = useRef<HTMLVideoElement | null>(null);
  /** The leader in state too, so the pref effects re-apply when a source switch
   *  hands the audio to another decoder. */
  const [leaderEl, setLeaderEl] = useState<HTMLVideoElement | null>(null);
  /** The boxes in the frames the elements are moved into. */
  const mainHostRef = useRef<HTMLDivElement>(null);
  const secondHostRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const videoAreaRef = useRef<HTMLDivElement>(null);
  const togglePlayRef = useRef<() => void>(() => {});
  // The key handler registers once and reaches actions through refs.
  const toggleTranscriptRef = useRef<() => void>(() => {});
  const toggleCaptionsRef = useRef<() => void>(() => {});
  // `following` mirrored for pointer handlers, which must not re-subscribe.
  const followingRef = useRef(true);
  const hideControlsRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Three independent holds on the bar: pointer on it, a panel open, a volume
  // drag that has left it.
  const pointerOnControlsRef = useRef(false);
  const panelOpenRef = useRef(false);
  const volumeDraggingRef = useRef(false);
  const controlsHeld = () =>
    pointerOnControlsRef.current ||
    panelOpenRef.current ||
    volumeDraggingRef.current;

  const {
    dock,
    height,
    width,
    size,
    resizing,
    dropTarget,
    startDockDrag,
    startResize,
  } = useTranscriptDock(containerRef);

  // ── Sources ──────────────────────────────────────────────────────────────

  // Served over localhost HTTP, not convertFileSrc — WebKit's media stack
  // refuses custom-scheme (asset://) sources outright. See lib/media.ts.
  const [urls, setUrls] = useState<Record<SourceNum, string | null>>({
    1: null,
    2: null,
  });
  useEffect(() => {
    let stale = false;
    Promise.all(
      SOURCES.map(async (n) => {
        const path = videoPathFor(lecture, n);
        return [n, path ? await mediaSrc(path) : null] as const;
      }),
    ).then((pairs) => {
      if (!stale) {
        setUrls({ 1: pairs[0][1], 2: pairs[1][1] });
      }
    });
    return () => {
      stale = true;
    };
  }, [lecture.video_path, lecture.video2_path]); // eslint-disable-line react-hooks/exhaustive-deps

  /** Downloaded / downloading, per source, for the two source controls. */
  const sources: SourceStates = useMemo(() => {
    const of = (n: SourceNum): SourceState => {
      const p = downloads.progress[dlKey(lecture.id, n)] ?? null;
      return {
        ready: !!videoPathFor(lecture, n),
        busy: isDownloading(downloads, lecture.id, n),
        percent: p?.percent ?? 0,
        phase: p?.phase ?? "",
      };
    };
    return { 1: of(1), 2: of(2) };
  }, [downloads, lecture]);

  /** Echo360 publishes a camera stream for this capture (`docs/sync.md`). */
  const hasSecondSource = lecture.has_source2 === 1;

  // A two-frame layout needs both files; until then it is one screen.
  const layout = urls[1] && urls[2] ? layoutPref : "single";
  /** The stream in the main frame; the pref holds only while it is on disk. */
  const mainSource: SourceNum = urls[mainPref] ? mainPref : urls[2] && !urls[1] ? 2 : 1;
  const otherSource: SourceNum = mainSource === 1 ? 2 : 1;
  const mainSrc = urls[mainSource];

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

  const {
    pipStyle,
    split,
    pipDragging,
    splitting,
    startPipMove,
    startPipResize,
    startSplitDrag,
  } = useSourceLayout(videoAreaRef, pipAspect);

  /** Which frame has its source pill open, so it stays put while it is. */
  const [openSwitcher, setOpenSwitcher] = useState<SourceNum | null>(null);

  // ── Chapters ─────────────────────────────────────────────────────────────

  // Read from SQLite, not the `lecture` row: in the side panel that row is a
  // store snapshot a chaptering run outlives.
  const chapterState = useLectureChapters(lecture.id);
  const chapterStarts = useMemo(
    () => chapterState.chapters.map((c) => c.start_seconds),
    [chapterState.chapters],
  );

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

  // Reset per lecture: fresh transcript, restored progress position.
  useEffect(() => {
    setCues([]);
    setActiveCueIdx(-1);
    setCurrentTime(0);
    setFileDuration(0);
    setIsPlaying(false);
    setError(null);
    setFollowing(true);
    followingRef.current = true;

    if (lecture.transcript_path) loadTranscript(lecture.transcript_path);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lecture.id, lecture.transcript_path, lecture.video_path]);

  // Where playback resumes is `lib/lecturePlayback.ts`'s call.
  const handleLoadedMetadata = () => {
    const v = videoRef.current;
    if (!v) return;
    // A stream still being sized reports Infinity or NaN; keep the fallback.
    if (Number.isFinite(v.duration) && v.duration > 0) setFileDuration(v.duration);
  };

  /** What every clock in the player counts against. */
  const duration = fileDuration || lecture.duration_seconds;

  // ── Controls visibility ──────────────────────────────────────────────────

  // Playing, the bar fades after idle seconds; paused, it stays. The timer reads
  // the element, not `isPlaying`, so it never needs re-arming.
  const revealControls = useCallback(() => {
    setControlsVisible(true);
    if (hideControlsRef.current) clearTimeout(hideControlsRef.current);
    hideControlsRef.current = setTimeout(() => {
      if (controlsHeld()) return;
      if (videoRef.current && !videoRef.current.paused) setControlsVisible(false);
    }, 2200);
  }, []);

  useEffect(() => {
    if (isPlaying) revealControls();
    else {
      if (hideControlsRef.current) clearTimeout(hideControlsRef.current);
      setControlsVisible(true);
    }
  }, [isPlaying, revealControls]);

  useEffect(
    () => () => {
      if (hideControlsRef.current) clearTimeout(hideControlsRef.current);
    },
    [],
  );

  // ── Fullscreen ───────────────────────────────────────────────────────────

  // Not `requestFullscreen()`: WKWebView gates element fullscreen behind
  // Tauri's `macos-private-api`, and even then shows only the element's subtree,
  // cutting off every Radix popup portalled to `document.body`. So the player is
  // a fixed overlay over a window-fullscreened app. Entering takes the window
  // fullscreen too; leaving the overlay keeps the window's; leaving the
  // window's fullscreen leaves both.
  const isFullscreenRef = useRef(false);
  isFullscreenRef.current = isFullscreen;

  const handleToggleFullscreen = useCallback(async () => {
    if (!allowFullscreen) return;
    const next = !isFullscreenRef.current;
    setIsFullscreen(next);
    if (!next) return;
    try {
      const win = getCurrentWindow();
      if (!(await win.isFullscreen())) await win.setFullscreen(true);
    } catch {
      /* not fatal — the overlay just covers a windowed app */
    }
  }, [allowFullscreen]);

  // Leaving window fullscreen another way (green button, ⌃⌘F) arrives as a
  // resize; only the leaving matters.
  useEffect(() => {
    const win = getCurrentWindow();
    const unlisten = win.onResized(async () => {
      if (!isFullscreenRef.current) return;
      try {
        if (!(await win.isFullscreen())) setIsFullscreen(false);
      } catch {
        /* ignore */
      }
    });
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, []);

  // ── Keyboard shortcuts ───────────────────────────────────────────────────

  useEffect(() => {
    if (!onScreen) return;
    const handler = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const tag = target?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      // An open popover (speed, layout, source) owns its arrows and space bar.
      if (target?.closest('[data-slot="popover-content"]')) return;

      switch (e.key) {
        case " ":
          e.preventDefault();
          togglePlayRef.current();
          break;
        case "ArrowLeft":
          e.preventDefault();
          if (videoRef.current)
            videoRef.current.currentTime = Math.max(
              0,
              videoRef.current.currentTime - 5,
            );
          break;
        case "ArrowRight":
          e.preventDefault();
          if (videoRef.current) videoRef.current.currentTime += 5;
          break;
        case "f":
          if (!e.ctrlKey && !e.metaKey && !e.altKey) {
            e.preventDefault();
            handleToggleFullscreen();
          }
          break;
        // Bare keys only: ⌘T opens a tab and ⌃C is a copy on some layouts.
        case "c":
          if (!e.ctrlKey && !e.metaKey && !e.altKey) {
            e.preventDefault();
            toggleCaptionsRef.current();
          }
          break;
        case "t":
          if (!e.ctrlKey && !e.metaKey && !e.altKey) {
            e.preventDefault();
            toggleTranscriptRef.current();
          }
          break;
        case "Escape":
          if (isFullscreenRef.current) {
            e.preventDefault();
            handleToggleFullscreen();
          }
          break;
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handleToggleFullscreen, onScreen]);

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
    }
  };

  // T does what the dock button does: parse a transcript on disk but not loaded,
  // fetch one if the *preference* is the transcript tab (it outlives
  // `tabInFront` dropping that tab), otherwise show or hide the dock.
  const toggleTranscript = () => {
    if (!allowDock) return;
    if (lecture.transcript_path && cues.length === 0) {
      loadTranscript(lecture.transcript_path);
    } else if (!lecture.transcript_path && dockTab === "transcript") {
      handleDownloadTranscript();
    } else {
      setPrefs({ transcriptVisible: !transcriptVisible });
    }
  };
  toggleTranscriptRef.current = toggleTranscript;

  // Captions come from the cues: no transcript, nothing to toggle.
  const toggleCaptions = () => {
    if (!lecture.transcript_path) return;
    setPrefs({ captionsEnabled: !captionsEnabled });
  };
  toggleCaptionsRef.current = toggleCaptions;

  // ── Playback ─────────────────────────────────────────────────────────────

  const handleTimeUpdate = () => {
    const v = videoRef.current;
    if (!v) return;
    const t = v.currentTime;
    setCurrentTime(t);
    atRef.current = t;

    let idx = -1;
    for (let i = cues.length - 1; i >= 0; i--) {
      if (t >= cues[i].start) {
        idx = i;
        break;
      }
    }
    setActiveCueIdx(idx);
  };

  // ── Follow ───────────────────────────────────────────────────────────────

  // `FollowList` does the scrolling; the player owns the flag and the ways back
  // to live, shared by the Transcript and Read tabs.
  const handleScrollAway = useCallback(() => {
    if (!followingRef.current) return;
    followingRef.current = false;
    setFollowing(false);
  }, []);

  const handleBackToLive = useCallback(() => {
    if (followingRef.current) return;
    followingRef.current = true;
    setFollowing(true);
  }, []);

  const handleCueSeek = useCallback((seconds: number) => {
    if (videoRef.current) videoRef.current.currentTime = seconds;
  }, []);

  const togglePlay = () => {
    const v = videoRef.current;
    if (!v) return;
    if (v.paused) {
      v.play();
      setIsPlaying(true);
    } else {
      v.pause();
      setIsPlaying(false);
    }
  };
  togglePlayRef.current = togglePlay;

  // `playbackRate` resets when a new source loads, hence an effect.
  useEffect(() => {
    if (leaderEl) leaderEl.playbackRate = speed;
  }, [speed, leaderEl]);

  // Per-element too; mute is independent of volume, so unmuting restores it.
  useEffect(() => {
    if (!leaderEl) return;
    leaderEl.volume = volume;
    leaderEl.muted = muted;
  }, [volume, muted, leaderEl]);

  // ── The shared element ───────────────────────────────────────────────────

  // The element outlives this component (`lib/lecturePlayback.ts`), so it is
  // adopted, not rendered; listeners reach the current handlers through a ref.
  const mediaRef = useRef({
    timeUpdate: handleTimeUpdate,
    loadedMetadata: handleLoadedMetadata,
    togglePlay,
    backToLive: handleBackToLive,
  });
  mediaRef.current = {
    timeUpdate: handleTimeUpdate,
    loadedMetadata: handleLoadedMetadata,
    togglePlay,
    backToLive: handleBackToLive,
  };

  useEffect(() => {
    const mainHost = mainHostRef.current;
    // Behind another tab the elements stay with whoever has them; this player
    // re-adopts them when its pane comes forward.
    if (!mainHost || !mainSrc || !onScreen) {
      videoRef.current = null;
      setLeaderEl(null);
      return;
    }

    // The main frame is first, and first is the leader: its audio is what plays.
    const plan: SourcePlan[] = [{ source: mainSource, src: mainSrc, host: mainHost }];
    const secondHost = secondHostRef.current;
    const secondSrc = urls[otherSource];
    if (layout !== "single" && secondHost && secondSrc) {
      plan.push({ source: otherSource, src: secondSrc, host: secondHost });
    }

    const v = syncLectureSources(lecture, plan, { tab: tabId, host });
    // Parking restores the elements only if still ours — another player (e.g.
    // expand-to-tab) may have taken them since.
    const claim = playbackClaim();
    if (!v) return;
    videoRef.current = v;
    setLeaderEl(v);

    // Read here: the inset element exists only once the plan is reconciled.
    const inset = plan.length > 1 ? videoForSource(plan[1].source) : null;
    const readAspect = () => {
      if (inset?.videoWidth && inset.videoHeight) {
        setPipAspect(inset.videoWidth / inset.videoHeight);
      }
    };
    readAspect();
    inset?.addEventListener("loadedmetadata", readAspect);

    // A lecture that kept playing in the background: state follows the element.
    setIsPlaying(!v.paused);
    if (Number.isFinite(v.duration) && v.duration > 0) setFileDuration(v.duration);
    mediaRef.current.timeUpdate();

    const onTimeUpdate = () => mediaRef.current.timeUpdate();
    const onLoadedMetadata = () => mediaRef.current.loadedMetadata();
    const onClick = () => {
      if (panelOnScreen()) return;
      mediaRef.current.togglePlay();
    };
    const onPlay = () => {
      setIsPlaying(true);
      // Pressing play is a request to be back where the video is.
      mediaRef.current.backToLive();
    };
    const onPause = () => setIsPlaying(false);
    const onEnded = () => setIsPlaying(false);
    const onError = () => {
      const err = v.error;
      setError(
        `Video failed to load (code ${err?.code ?? "?"}: ${
          err?.message || "unknown"
        })`,
      );
    };

    v.addEventListener("timeupdate", onTimeUpdate);
    v.addEventListener("loadedmetadata", onLoadedMetadata);
    v.addEventListener("click", onClick);
    v.addEventListener("play", onPlay);
    v.addEventListener("pause", onPause);
    v.addEventListener("ended", onEnded);
    v.addEventListener("error", onError);

    return () => {
      v.removeEventListener("timeupdate", onTimeUpdate);
      v.removeEventListener("loadedmetadata", onLoadedMetadata);
      v.removeEventListener("click", onClick);
      v.removeEventListener("play", onPlay);
      v.removeEventListener("pause", onPause);
      v.removeEventListener("ended", onEnded);
      v.removeEventListener("error", onError);
      inset?.removeEventListener("loadedmetadata", readAspect);
      videoRef.current = null;
      // Unmounting is a tab switch, not a stop: the elements go back to their
      // off-screen host and carry on. Closing the lecture is what stops them.
      parkLectureVideos(claim);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lecture.id, urls[1], urls[2], layout, mainSource, onScreen, tabId, host]);

  // Progress is saved by the module, even while nothing is mounted.
  useWindowEvent(LECTURE_PROGRESS_EVENT, () => onRefresh());

  // ── Render ───────────────────────────────────────────────────────────────

  // The dock stays mounted so it can slide in and out.
  const hasTranscript = cues.length > 0;
  const showDock = allowDock && transcriptVisible;
  /** The tab the dock is actually showing — what T shows or hides. */
  const frontTab = tabInFront(dockTab, hasTranscript);

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
      onSeek: handleCueSeek,
      onFind: chapterState.find,
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
      duration,
      lecture.video_path,
      handleCueSeek,
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
      onScrollAway: handleScrollAway,
      onBackToLive: handleBackToLive,
      onSeek: handleCueSeek,
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
      handleScrollAway,
      handleBackToLive,
      handleCueSeek,
    ],
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
    [lecture.id, buildMoment],
  );

  return (
    // The dock side is a flex direction: `*-reverse` puts the panel before the
    // video stack visually while leaving the divider between the two.
    <div
      ref={containerRef}
      className={cn(
        "flex overflow-hidden min-h-0 min-w-0 bg-background relative",
        // Over the sidebar, the tab strip and the peek panel (z-20) alike.
        isFullscreen ? "fixed inset-0 z-50" : "flex-1",
        dock === "bottom" && "flex-col",
        dock === "top" && "flex-col-reverse",
        dock === "right" && "flex-row",
        dock === "left" && "flex-row-reverse",
      )}
    >
      <div className="flex-1 flex flex-col min-h-0 min-w-0 overflow-hidden">
        {/* Video area — the controls live over the frame, YouTube-style */}
        <div
          ref={videoAreaRef}
          className={cn(
            "flex-1 bg-black flex flex-col min-h-0 relative overflow-hidden",
            !controlsVisible && "cursor-none",
          )}
          onPointerMove={revealControls}
          onPointerLeave={() => {
            pointerOnControlsRef.current = false;
            if (controlsHeld()) return;
            if (videoRef.current && !videoRef.current.paused) {
              setControlsVisible(false);
            }
          }}
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
                        "absolute z-10 size-5 touch-none transition-opacity",
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
          {captionsEnabled && activeCueIdx >= 0 && (
            <CaptionOverlay
              text={cues[activeCueIdx]?.text ?? ""}
              boundsRef={videoAreaRef}
              // Lift clear of the control bar while it shows.
              lift={controlsVisible ? 44 : 0}
            />
          )}

          {/* Controls — one scrim, the scrub bar across the top of it */}
          <div
            className={cn(
              "absolute inset-x-0 bottom-0 z-30 transition-opacity duration-200",
              controlsVisible ? "opacity-100" : "opacity-0 pointer-events-none",
            )}
            onPointerEnter={() => {
              pointerOnControlsRef.current = true;
              revealControls();
            }}
            onPointerLeave={() => {
              pointerOnControlsRef.current = false;
              revealControls();
            }}
          >
            <div className="pointer-events-none absolute inset-x-0 bottom-0 h-24 bg-gradient-to-t from-black/85 via-black/45 to-transparent" />

            <div className="relative px-3 pb-1">
              {/* The playing chapter's name — the one thing the scrub bar cannot say. */}
              {activeChapter && (
                <div className="select-none truncate pb-1 text-[11.5px] font-medium text-white/90">
                  {activeChapter.title}
                </div>
              )}

              <SeekBar
                value={Math.min(currentTime, duration)}
                max={duration || 1}
                previewSrc={mainSrc}
                chapters={chapterStarts}
                onSeek={(v) => {
                  setCurrentTime(v);
                  if (videoRef.current) videoRef.current.currentTime = v;
                }}
              />

              <div className="flex h-9 items-center gap-0.5">
                <ControlButton
                  label={isPlaying ? "Pause" : "Play"}
                  onClick={togglePlay}
                  disabled={!mainSrc}
                >
                  {isPlaying ? (
                    <Pause size={16} weight="fill" />
                  ) : (
                    <Play size={16} weight="fill" />
                  )}
                </ControlButton>

                <VolumeControl
                  volume={volume}
                  muted={muted}
                  onChange={(v) =>
                    // Dragging up from silence is a request to hear it.
                    setPrefs({ volume: v, muted: muted && v === 0 })
                  }
                  onToggleMute={() => setPrefs({ muted: !muted })}
                  onDraggingChange={(dragging) => {
                    volumeDraggingRef.current = dragging;
                    revealControls();
                  }}
                />

                <span className="ml-1 select-none whitespace-nowrap text-[11.5px] tabular-nums text-white/85">
                  {fmtClockSecs(Math.floor(currentTime), duration >= 3600)}{" "}
                  <span className="text-white/45">/</span>{" "}
                  {fmtClockSecs(duration)}
                </span>

                <div className="flex-1" />

                <SpeedControl
                  speed={speed}
                  onChange={(s) => setPrefs({ speed: s })}
                  onOpenChange={(open) => {
                    panelOpenRef.current = open;
                    revealControls();
                  }}
                />

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
                    onOpenChange={(open) => {
                      panelOpenRef.current = open;
                      revealControls();
                    }}
                  />
                )}

                {/* Labelled by the tab in front: it folds the whole dock. */}
                {allowDock && (
                  <ControlButton
                    label={
                      !lecture.transcript_path && dockTab === "transcript"
                        ? "Download transcript"
                        : `${showDock ? "Hide" : "Show"} ${DOCK_TAB_NOUN[frontTab]} (T)`
                    }
                    active={showDock}
                    onClick={toggleTranscript}
                  >
                    <SidebarSimple
                      size={16}
                      className={cn("transition-transform", DOCK_ICON_FACING[dock])}
                    />
                  </ControlButton>
                )}

                <ControlButton
                  label={
                    captionsEnabled ? "Hide captions (C)" : "Show captions (C)"
                  }
                  active={captionsEnabled}
                  disabled={!lecture.transcript_path}
                  onClick={toggleCaptions}
                >
                  <ClosedCaptioning
                    size={17}
                    weight={captionsEnabled ? "fill" : "regular"}
                  />
                </ControlButton>

                {allowFullscreen && (
                  <ControlButton
                    label={isFullscreen ? "Exit fullscreen (F)" : "Fullscreen (F)"}
                    onClick={handleToggleFullscreen}
                  >
                    {isFullscreen ? <ArrowsIn size={16} /> : <ArrowsOut size={16} />}
                  </ControlButton>
                )}
              </div>
            </div>
          </div>
        </div>

        {error && (
          <div className="shrink-0 px-3 py-1.5 text-[11px] text-destructive border-t border-border bg-destructive/5 break-words">
            {error}
          </div>
        )}
      </div>

      {/* The dock. Not rendered where disallowed, so its Chat tab does not load
          a thread for every peeked lecture. */}
      {allowDock && (
        <>
          {showDock && <DockResizeHandle dock={dock} onPointerDown={startResize} />}
          <TranscriptPanel
            cues={cues}
            activeCueIdx={activeCueIdx}
            tab={dockTab}
            onTabChange={(t) => setPrefs({ dockTab: t })}
            chapters={chaptersProps}
            reading={readingProps}
            chat={chatProps}
            dock={dock}
            size={size}
            open={showDock}
            resizing={resizing}
            onSeek={handleCueSeek}
            onClose={() => setPrefs({ transcriptVisible: false })}
            onHeaderPointerDown={startDockDrag}
            following={following}
            onScrollAway={handleScrollAway}
            onBackToLive={handleBackToLive}
          />

          {dropTarget && (
            <DockDropPreview dock={dropTarget} height={height} width={width} />
          )}
        </>
      )}
    </div>
  );
}
