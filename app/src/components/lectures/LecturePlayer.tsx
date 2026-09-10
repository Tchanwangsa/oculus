import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useBlocker } from "react-router-dom";
import {
  ArrowsIn,
  ArrowsOut,
  CircleNotch,
  ClosedCaptioning,
  DownloadSimple,
  FileText,
  Pause,
  Play,
} from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
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
  isLecturePlaying,
  ownsPlayback,
  parkLectureVideos,
  stopLecturePlayback,
  syncLectureSources,
  videoForSource,
  type SourcePlan,
} from "@/lib/lecturePlayback";
import { useTabStore } from "@/stores/tabStore";
import { confirmLeavingLecture } from "@/stores/leaveLectureStore";
import {
  parseVtt,
  fmtDuration,
  fmtTime,
  fmtLectureDate,
  type Cue,
} from "@/lib/lectures";
import { useTranscriptDock } from "@/hooks/useTranscriptDock";
import { PIP_CORNERS, useSourceLayout, type PipCorner } from "@/hooks/useSourceLayout";
import { usePlayerPrefs } from "@/stores/playerPrefsStore";
import {
  LayoutControl,
  SOURCES,
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
} from "@/components/lectures/TranscriptPanel";

/**
 * Is a panel on screen right now?
 *
 * The player is the one place in the app where a bare click on the background
 * *does* something — it plays or pauses — so it is the one place that has to
 * tell a click from a dismissal. Clicking off the source or layout panel used
 * to close it *and* toggle playback, which is one action too many.
 *
 * Radix defers its outside-dismissal to the `click` (not the pointerdown) and
 * handles it on `document`, so during the target phase — where the video's own
 * handler runs, and before the pointer even lifts for the scrub bar — the
 * panel is still up and still what the click was for. Which makes its presence
 * the whole test.
 *
 * `[data-state=open]` earns its keep: Radix keeps a closing panel mounted for
 * the length of its exit animation, and a click landing in those 150ms is a
 * real click.
 */
const panelOnScreen = () =>
  !!document.querySelector('[data-slot="popover-content"][data-state="open"]');

/**
 * Video scrub bar. Not the shadcn Slider: it mixes `clientX` with
 * `getBoundingClientRect`, which is one measurement more than this needs.
 * `offsetX / offsetWidth` stays entirely in the element's own coordinate
 * space, so a click lands exactly where the pointer is.
 *
 * It sits on the scrim over the video, so its colours are fixed rather than
 * themed: the ground is always the frame behind it, never `background`.
 */
function SeekBar({
  value,
  max,
  previewSrc,
  onSeek,
}: {
  value: number;
  max: number;
  /** Source for the hover thumbnail; `null` until the video is downloaded. */
  previewSrc: string | null;
  onSeek: (seconds: number) => void;
}) {
  const pct = Math.min(100, Math.max(0, (value / max) * 100));

  // The preview's position is kept while it fades out, so leaving the bar
  // doesn't make the thumbnail jump to the left edge on its way off screen.
  // `armed` mounts the second decoder on first hover rather than with the
  // player: an untouched scrub bar should not cost a metadata fetch.
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

/**
 * One button on the overlaid control bar. Not the shadcn `Button`: its ghost
 * variant hovers to `accent`, a near-white surface that vanishes on a video.
 * Over the scrim the whole palette is fixed white-on-frame.
 */
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
          {/* YouTube's underline. On a frame a lit icon reads the same as an
              unlit one, so an "on" toggle needs a mark of its own. */}
          {active && (
            <span className="absolute bottom-[3px] h-[2px] w-3.5 rounded-full bg-white" />
          )}
        </button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

/**
 * One picture in the player. The `<video>` itself is not rendered here — it is
 * a long-lived element moved into `hostRef` by `lib/lecturePlayback.ts` — so
 * this is the box around it plus the control that says which stream it shows.
 *
 * The switcher hides with the control bar rather than only on pointer-out: a
 * frame the pointer is resting on still counts as idle after a couple of
 * seconds, and a lone pill floating over a lecture with no bar under it reads
 * as a stuck overlay.
 */
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
  /** False for a capture with one stream — there is nothing to switch to, so
      no pill, rather than a control whose only option is the current one. */
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
  /**
   * Off in the peek panel. Fullscreen here is the *window's* — a peek is a
   * panel over a page that stays mounted behind it, so taking one fullscreen
   * would be a side panel eating the screen. Expand promotes it to its own
   * tab first, and the button is there.
   */
  allowFullscreen?: boolean;
}

/**
 * The whole lecture player — video, controls bar, transcript. Self-contained:
 * loads its own transcript, tracks its own download, saves its own progress.
 * Wrapped by the lectures-tab peek and the standalone full-page view alike.
 */
export function LecturePlayer({
  lecture,
  onRefresh,
  allowFullscreen = true,
}: LecturePlayerProps) {
  const [cues, setCues] = useState<Cue[]>([]);
  const [activeCueIdx, setActiveCueIdx] = useState(-1);
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  /**
   * The file's own length, which is not always the catalogue's. Echo360 reports
   * a lesson duration from its scheduling data, and the recording it hands over
   * runs a little past it — enough that the clock read `1:55:00 / 1:54:46` at
   * the end of a lecture. The element is the authority for anything about
   * playback; `lecture.duration_seconds` stands in until metadata arrives (and
   * for a lecture whose video has not been downloaded at all).
   */
  const [fileDuration, setFileDuration] = useState(0);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** Transcript list is tracking playback (vs. the user reading ahead). */
  const [following, setFollowing] = useState(true);
  /** Controls are over the frame, so they fade out of the way while playing. */
  const [controlsVisible, setControlsVisible] = useState(true);

  // Speed, captions and the transcript's side and size are preferences, not
  // per-lecture state: set once, they hold for every recording and survive a
  // restart. The per-lecture bit is the playback position, and that is in the
  // DB (`lectures.progress_seconds`).
  const speed = usePlayerPrefs((s) => s.speed);
  /** Playback belongs to the tab the player is mounted in; see below. */
  const activeTabId = useTabStore((s) => s.activeId);
  const volume = usePlayerPrefs((s) => s.volume);
  const muted = usePlayerPrefs((s) => s.muted);
  const captionsEnabled = usePlayerPrefs((s) => s.captionsEnabled);
  const transcriptVisible = usePlayerPrefs((s) => s.transcriptVisible);
  const layoutPref = usePlayerPrefs((s) => s.layout);
  const mainPref = usePlayerPrefs((s) => s.mainSource);
  const setPrefs = usePlayerPrefs((s) => s.set);

  // Global on purpose: a download outlives this component (close the peek,
  // reopen it — the same download is still running in Rust).
  const downloads = useLectureDownloads();
  const downloading = isDownloading(downloads, lecture.id);
  const dlProgress = downloads.progress[dlKey(lecture.id, 1)] ?? null;

  /** The leader element while this player has it; see lib/lecturePlayback.ts. */
  const videoRef = useRef<HTMLVideoElement | null>(null);
  /**
   * The leader in state as well as in a ref: the ref is what handlers read,
   * and the state is what makes the preference effects below re-apply speed
   * and volume when a source switch hands the audio to a different decoder.
   */
  const [leaderEl, setLeaderEl] = useState<HTMLVideoElement | null>(null);
  /** The boxes in the frames the elements are moved into. */
  const mainHostRef = useRef<HTMLDivElement>(null);
  const secondHostRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const videoAreaRef = useRef<HTMLDivElement>(null);
  const togglePlayRef = useRef<() => void>(() => {});
  // The keyboard listener is registered once, so what a key does reaches it
  // through a ref rather than through the effect's dependencies.
  const toggleTranscriptRef = useRef<() => void>(() => {});
  const toggleCaptionsRef = useRef<() => void>(() => {});
  // `following` mirrored for pointer handlers, which must not re-subscribe.
  const followingRef = useRef(true);
  const hideControlsRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Three reasons to pin the bar open, tracked apart because they end apart:
  // the pointer resting on it, one of its panels being open (speed, layout),
  // and a volume drag that has wandered off the bar.
  const pointerOnControlsRef = useRef(false);
  const panelOpenRef = useRef(false);
  const volumeDraggingRef = useRef(false);
  const controlsHeld = () =>
    pointerOnControlsRef.current ||
    panelOpenRef.current ||
    volumeDraggingRef.current;

  // Where the transcript sits (any edge of the player) and how big it is.
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

  // A two-frame layout needs two files, so it falls back to one screen rather
  // than half a player while the camera is still a download away.
  const layout = urls[1] && urls[2] ? layoutPref : "single";
  /** The stream in the main frame; the other frame gets the other one. The
   *  preference only holds while that stream is actually on disk — a camera
   *  that has not been downloaded cannot be the one screen you are watching. */
  const mainSource: SourceNum = urls[mainPref] ? mainPref : urls[2] && !urls[1] ? 2 : 1;
  const otherSource: SourceNum = mainSource === 1 ? 2 : 1;
  const mainSrc = urls[mainSource];

  /**
   * Choosing a source in *either* frame swaps the pair, because the two frames
   * always show the two streams — there is no state where both show the same
   * one. So one number says everything: which stream is in the main frame.
   *
   * Which is why the second frame needs the other half of that swap: picking
   * Source 2 *there* means Source 2 in that frame, so Source 1 in the main
   * one. Handing it `selectSource` read the choice as the main frame's and
   * inverted the panel — the row you ticked was the one you did not get.
   */
  const selectSource = (n: SourceNum) => setPrefs({ mainSource: n });
  const selectSecondSource = (n: SourceNum) =>
    setPrefs({ mainSource: n === 1 ? 2 : 1 });

  const handleDownloadSource = (n: SourceNum) => {
    setError(null);
    downloadLecture(lecture, n)
      .then(onRefresh)
      .catch((e) => setError(`Download failed: ${e}`));
  };

  /** The PIP box keeps the inset picture's own shape, not an assumed 16:9.
   *  Filled in by the reconcile effect below, which is where the element the
   *  dimensions come from is known to exist. */
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

  // Where playback resumes is `lib/lecturePlayback.ts`'s call, not this one:
  // it is the only thing that knows whether a load is a fresh lecture (restore
  // the saved second) or a source switch mid-lecture (carry the live one).
  const handleLoadedMetadata = () => {
    const v = videoRef.current;
    if (!v) return;
    // A stream still being sized reports Infinity or NaN; keep the fallback.
    if (Number.isFinite(v.duration) && v.duration > 0) setFileDuration(v.duration);
  };

  /** What every clock in the player counts against. */
  const duration = fileDuration || lecture.duration_seconds;

  // ── Controls visibility ──────────────────────────────────────────────────

  // Paused, the bar stays; playing, it fades after a couple of idle seconds
  // and any pointer movement over the frame brings it back. The timer reads
  // the element rather than `isPlaying` so it never has to be re-armed when
  // the state it is waiting on changes.
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

  // Not `requestFullscreen()`. WKWebView keeps element fullscreen behind a
  // private preference wry only sets under Tauri's `macos-private-api`, so the
  // call was rejected and the `.catch` swallowed it — the button did nothing.
  // Turning that feature on would work and then break something worse: WebKit
  // displays only the fullscreen element's subtree, and every Radix popup in
  // here (the speed panel, every tooltip) is portalled to `document.body`,
  // outside it. So fullscreen is the *window's*, with the player promoted to a
  // fixed overlay over the app — the whole document stays on screen and the
  // popups keep working.
  //
  // Two fullscreens, nested, not one:
  //
  //   * the *window's* — macOS fullscreen, the app filling the display with
  //     the sidebar and tab strip still there;
  //   * the *player's* — `isFullscreen` here, the fixed overlay that covers
  //     that furniture so only the lecture is left.
  //
  // Entering the player's takes the window with it, because a lecture over a
  // half-screen window is not what anyone means by fullscreen. Leaving it does
  // *not* bring the window back: the overlay lifts and the sidebar and tabs
  // are underneath, still filling the display. Leaving the window's, though,
  // leaves both — there is no reading of "un-fullscreen the app" that keeps a
  // lecture pinned over everything.
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

  // The green button and ⌃⌘F leave the window's fullscreen by another door;
  // resize is how that arrives. Only the leaving matters — entering window
  // fullscreen from the tab strip is not a request to hide the tab strip.
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
  }, [handleToggleFullscreen]);

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

  // What the transcript button does, so T does the same three-way: fetch the
  // transcript if this lecture has never had one, parse it if it is on disk
  // but not loaded, otherwise show or hide the panel.
  const toggleTranscript = () => {
    if (!lecture.transcript_path) {
      handleDownloadTranscript();
    } else if (cues.length === 0) {
      loadTranscript(lecture.transcript_path);
    } else {
      setPrefs({ transcriptVisible: !transcriptVisible });
    }
  };
  toggleTranscriptRef.current = toggleTranscript;

  // Captions are drawn from the same cues, so with no transcript there is
  // nothing to turn on — the button is disabled in that state, and C is too.
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

    let idx = -1;
    for (let i = cues.length - 1; i >= 0; i--) {
      if (t >= cues[i].start) {
        idx = i;
        break;
      }
    }
    setActiveCueIdx(idx);
  };

  // ── Transcript follow ────────────────────────────────────────────────────

  // The scrolling itself lives in the panel, which owns the virtualizer and so
  // is the only thing that knows where a cue sits. The player owns just the
  // flag and the two ways back to live.
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

  // `playbackRate` is per-element and resets when a new source loads, so the
  // preference is applied as an effect rather than only on change.
  useEffect(() => {
    if (leaderEl) leaderEl.playbackRate = speed;
  }, [speed, leaderEl]);

  // Volume and mute are per-element the same way, and set together: they are
  // independent on the element, which is what lets unmuting land back on the
  // level rather than on full.
  useEffect(() => {
    if (!leaderEl) return;
    leaderEl.volume = volume;
    leaderEl.muted = muted;
  }, [volume, muted, leaderEl]);

  // ── The shared element ───────────────────────────────────────────────────

  // The video element outlives this component (`lib/lecturePlayback.ts`), so
  // it is adopted into the frame rather than rendered, and its listeners are
  // attached by hand. They reach the current handlers through a ref: these are
  // recreated on every render, and re-binding six listeners whenever a cue
  // index changes is churn for no gain.
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
    if (!mainHost || !mainSrc) {
      videoRef.current = null;
      setLeaderEl(null);
      return;
    }

    // Restate the whole layout; the module works out what that means for the
    // elements it owns. The main frame is first, and first is the leader — the
    // picture you are watching is the audio you hear.
    const plan: SourcePlan[] = [{ source: mainSource, src: mainSrc, host: mainHost }];
    const secondHost = secondHostRef.current;
    const secondSrc = urls[otherSource];
    if (layout !== "single" && secondHost && secondSrc) {
      plan.push({ source: otherSource, src: secondSrc, host: secondHost });
    }

    const v = syncLectureSources(lecture, plan, activeTabId);
    if (!v) return;
    videoRef.current = v;
    setLeaderEl(v);

    // The inset's shape comes from the picture in it. Read here rather than in
    // an effect of its own: the element only exists once the plan above has
    // been reconciled, and an effect that ran before it would find nothing and
    // never look again.
    const inset = plan.length > 1 ? videoForSource(plan[1].source) : null;
    const readAspect = () => {
      if (inset?.videoWidth && inset.videoHeight) {
        setPipAspect(inset.videoWidth / inset.videoHeight);
      }
    };
    readAspect();
    inset?.addEventListener("loadedmetadata", readAspect);

    // A lecture that kept playing while its tab was gone is already somewhere
    // when this mounts, and this component's state starts empty — so the state
    // takes the element's, not the other way round.
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
      parkLectureVideos();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lecture.id, urls[1], urls[2], layout, mainSource, activeTabId]);

  // ── Leaving ──────────────────────────────────────────────────────────────

  // Switching tabs leaves the lecture playing behind a tab you can get back
  // to. Navigating *this* tab somewhere else does not — the lecture would be
  // playing with nothing on screen owning it — so that one asks first.
  //
  // The check is which tab is active at the moment of the navigation: the
  // strip sets the destination tab active before it navigates, so a tab switch
  // is already "some other tab" here, while a sidebar click from the lecture's
  // own tab is still this one.
  const blocker = useBlocker(
    ({ currentLocation, nextLocation }) =>
      isLecturePlaying() &&
      ownsPlayback(useTabStore.getState().activeId) &&
      currentLocation.pathname + currentLocation.search !==
        nextLocation.pathname + nextLocation.search,
  );

  useEffect(() => {
    if (blocker.state !== "blocked") return;
    confirmLeavingLecture(
      () => {
        stopLecturePlayback();
        blocker.proceed();
      },
      // Either way the blocker has to be released, or the router stays stuck
      // on a navigation nobody is going to answer twice.
      () => blocker.reset(),
    );
  }, [blocker]);

  // Progress is written by the module, including while nothing is mounted;
  // the list this player sits in still wants to hear about it.
  useEffect(() => {
    const onSaved = () => onRefresh();
    window.addEventListener(LECTURE_PROGRESS_EVENT, onSaved);
    return () => window.removeEventListener(LECTURE_PROGRESS_EVENT, onSaved);
  }, [onRefresh]);

  // ── Render ───────────────────────────────────────────────────────────────

  // Mounted whenever there is a transcript, shown when the preference says so:
  // the panel slides in and out, and a slide needs both ends on screen.
  const hasTranscript = cues.length > 0;
  const showTranscript = transcriptVisible && hasTranscript;

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
      {/* Video + controls stack */}
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
              {/* `flexGrow` on a zero basis, not a percentage height: the two
                  screens divide what is left *after* the divider, so the split
                  can never add up to more than the frame. */}
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
                  // Under the control bar (z-30) so it can never cover the
                  // scrub bar, over the main picture so it is visible at all.
                  className={cn(
                    "absolute z-20 touch-none rounded-lg border border-white/20 bg-black",
                    "shadow-2xl shadow-black/60",
                    pipDragging ? "cursor-grabbing" : "cursor-grab",
                  )}
                  style={pipStyle}
                  onPointerDown={startPipMove}
                >
                  {/* On the switcher pill's terms, not hover's alone: four
                      white pips sitting on the inset after the bar has faded
                      read as furniture stuck to the picture. A drag pins them,
                      since a handle that vanishes under the pointer you are
                      resizing with is the one moment they must not go. */}
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
                {fmtLectureDate(lecture.date)} · {fmtDuration(lecture.duration_seconds)}
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
              // Captions default to the bottom of the frame, which is exactly
              // where the bar fades in. Lift them clear while it is there.
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
              <SeekBar
                value={Math.min(currentTime, duration)}
                max={duration || 1}
                previewSrc={mainSrc}
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
                  {fmtTime(Math.floor(currentTime), duration >= 3600)}{" "}
                  <span className="text-white/45">/</span>{" "}
                  {fmtTime(duration)}
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

                <ControlButton
                  label={
                    lecture.transcript_path
                      ? transcriptVisible
                        ? "Hide transcript (T)"
                        : "Show transcript (T)"
                      : "Download transcript"
                  }
                  active={!!lecture.transcript_path && transcriptVisible}
                  onClick={toggleTranscript}
                >
                  <FileText
                    size={16}
                    weight={
                      lecture.transcript_path && transcriptVisible
                        ? "fill"
                        : "regular"
                    }
                  />
                </ControlButton>

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

      {/* Transcript panel — dragged to any edge, dragged wider from its divider */}
      {hasTranscript && (
        <>
          {showTranscript && (
            <DockResizeHandle dock={dock} onPointerDown={startResize} />
          )}
          <TranscriptPanel
            cues={cues}
            activeCueIdx={activeCueIdx}
            dock={dock}
            size={size}
            open={showTranscript}
            resizing={resizing}
            onSeek={handleCueSeek}
            onHeaderPointerDown={startDockDrag}
            following={following}
            onScrollAway={handleScrollAway}
            onBackToLive={handleBackToLive}
          />
        </>
      )}

      {dropTarget && (
        <DockDropPreview dock={dropTarget} height={height} width={width} />
      )}
    </div>
  );
}
