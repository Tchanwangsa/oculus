import { useCallback, useEffect, useRef, useState } from "react";
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
  downloadLecture,
  isDownloading,
  useLectureDownloads,
} from "@/stores/lectureDownloadStore";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { mediaSrc } from "@/lib/media";
import { updateLectureTranscriptPath, type Lecture } from "@/lib/db";
import {
  LECTURE_PROGRESS_EVENT,
  adoptLectureVideo,
  isLecturePlaying,
  lectureVideo,
  openLecture,
  ownsPlayback,
  parkLectureVideo,
  stopLecturePlayback,
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
import { usePlayerPrefs } from "@/stores/playerPrefsStore";
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
  const setPrefs = usePlayerPrefs((s) => s.set);

  // Global on purpose: a download outlives this component (close the peek,
  // reopen it — the same download is still running in Rust).
  const downloads = useLectureDownloads();
  const downloading = isDownloading(downloads, lecture.id);
  const dlProgress = downloads.progress[lecture.id] ?? null;

  /** The shared element while this player has it; see lib/lecturePlayback.ts. */
  const videoRef = useRef<HTMLVideoElement | null>(null);
  /** The box in the frame the element is moved into. */
  const videoHostRef = useRef<HTMLDivElement>(null);
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
  // the pointer resting on it, a panel of its own being open, and a volume
  // drag that has wandered off the bar.
  const pointerOnControlsRef = useRef(false);
  const speedPanelOpenRef = useRef(false);
  const volumeDraggingRef = useRef(false);
  const controlsHeld = () =>
    pointerOnControlsRef.current ||
    speedPanelOpenRef.current ||
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

  // Served over localhost HTTP, not convertFileSrc — WebKit's media stack
  // refuses custom-scheme (asset://) sources outright. See lib/media.ts.
  const [videoSrc, setVideoSrc] = useState<string | null>(null);
  useEffect(() => {
    let stale = false;
    if (!lecture.video_path) {
      setVideoSrc(null);
      return;
    }
    mediaSrc(lecture.video_path).then((url) => {
      if (!stale) setVideoSrc(url);
    });
    return () => {
      stale = true;
    };
  }, [lecture.video_path]);

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

  // Restore saved progress once the (async-resolved) source has metadata.
  const handleLoadedMetadata = () => {
    const v = videoRef.current;
    if (!v) return;
    // A stream still being sized reports Infinity or NaN; keep the fallback.
    if (Number.isFinite(v.duration) && v.duration > 0) setFileDuration(v.duration);
    if (lecture.progress_seconds > 5) v.currentTime = lecture.progress_seconds;
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
      // An open popover (the speed panel) owns its own arrows and space bar.
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
  const handleDownloadVideo = async () => {
    setError(null);
    try {
      await downloadLecture(lecture);
      onRefresh();
    } catch (e) {
      setError(`Download failed: ${e}`);
    }
  };

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
    if (videoRef.current) videoRef.current.playbackRate = speed;
  }, [speed, videoSrc]);

  // Volume and mute are per-element the same way, and set together: they are
  // independent on the element, which is what lets unmuting land back on the
  // level rather than on full.
  useEffect(() => {
    const v = videoRef.current;
    if (!v) return;
    v.volume = volume;
    v.muted = muted;
  }, [volume, muted, videoSrc]);

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
    const host = videoHostRef.current;
    if (!host || !videoSrc) {
      videoRef.current = null;
      return;
    }

    const v = lectureVideo();
    videoRef.current = v;
    openLecture(lecture, videoSrc);
    adoptLectureVideo(host, activeTabId);

    // A lecture that kept playing while its tab was gone is already somewhere
    // when this mounts, and this component's state starts empty — so the state
    // takes the element's, not the other way round.
    setIsPlaying(!v.paused);
    if (Number.isFinite(v.duration) && v.duration > 0) setFileDuration(v.duration);
    mediaRef.current.timeUpdate();

    const onTimeUpdate = () => mediaRef.current.timeUpdate();
    const onLoadedMetadata = () => mediaRef.current.loadedMetadata();
    const onClick = () => mediaRef.current.togglePlay();
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
      videoRef.current = null;
      // Unmounting is a tab switch, not a stop: the element goes back to its
      // off-screen host and carries on. Closing the lecture is what stops it.
      parkLectureVideo();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [videoSrc, lecture.id, activeTabId]);

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
          {videoSrc ? (
            /* Empty on purpose: the shared element is moved in here. */
            <div ref={videoHostRef} className="w-full h-full min-h-0" />
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
                previewSrc={videoSrc}
                onSeek={(v) => {
                  setCurrentTime(v);
                  if (videoRef.current) videoRef.current.currentTime = v;
                }}
              />

              <div className="flex h-9 items-center gap-0.5">
                <ControlButton
                  label={isPlaying ? "Pause" : "Play"}
                  onClick={togglePlay}
                  disabled={!videoSrc}
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
                    speedPanelOpenRef.current = open;
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
