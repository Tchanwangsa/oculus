/**
 * The app's one media player: the clock, the control bar over the frame,
 * fullscreen, keys, captions and the dock's frame. Callers own the element and
 * what the dock shows — a lecture adopts the shared elements
 * (`lib/lecturePlayback.ts`) and docks chapters and chat beside its
 * transcript; a library video renders its own `<video>`.
 *
 * `useMediaPlayer` holds the state, so a caller can derive from the playhead
 * (a chapter, a reading line) before rendering `MediaPlayer` with it.
 */
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  ArrowsIn,
  ArrowsOut,
  ClosedCaptioning,
  Pause,
  Play,
  SidebarSimple,
} from "@phosphor-icons/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { cn } from "@/lib/utils";
import { fmtClockSecs, spanAt, type Cue } from "@/lib/media";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useActivePaneId } from "@/stores/tabStore";
import { useTabId } from "@/components/tabs/TabContext";
import { useTranscriptDock, type Dock } from "@/hooks/useTranscriptDock";
import { usePlayerPrefs, type DockTab } from "@/stores/playerPrefsStore";
import { CaptionOverlay } from "@/components/media/CaptionOverlay";
import { ScrubPreview } from "@/components/media/ScrubPreview";
import { SpeedControl } from "@/components/media/SpeedControl";
import { VolumeControl } from "@/components/media/VolumeControl";
import { DockDropPreview, DockResizeHandle } from "@/components/media/MediaDock";

/** Turns `SidebarSimple` to face the dock's edge; the glyph's divider is on
 *  the left, so left is unrotated. */
const DOCK_ICON_FACING: Record<Dock, string> = {
  left: "",
  right: "rotate-180",
  top: "rotate-90",
  bottom: "-rotate-90",
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
  endAt,
  onSeek,
}: {
  value: number;
  max: number;
  /** Source for the hover thumbnail; `null` until the video is downloaded. */
  previewSrc: string | null;
  /** Chapter boundaries, in seconds — notched into the track. */
  chapters: number[];
  /** Where the content ends, in seconds, when known — a flag on the track
   *  with what follows dimmed. */
  endAt: number | null;
  onSeek: (seconds: number) => void;
}) {
  const endPct = endAt != null && endAt > 0 && endAt < max ? (endAt / max) * 100 : null;
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
        {endPct != null && (
          <div
            aria-hidden
            className="absolute inset-y-0 right-0 bg-black/35"
            style={{ left: `${endPct}%` }}
          />
        )}
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
      {/* Outside the track, which clips: the flag stands proud of it. */}
      {endPct != null && (
        <span
          aria-hidden
          className="pointer-events-none absolute h-[9px] w-[2px] -translate-x-1/2 rounded-full bg-white shadow-sm"
          style={{ left: `${endPct}%` }}
        />
      )}
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
export function ControlButton({
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

const NO_STARTS: number[] = [];

export interface MediaPlayerOptions {
  cues: Cue[];
  /** Changes when the player shows another recording: the clock, play state,
   *  follow and the element's error start over. */
  resetKey: string;
  /** The recording's length until the file reports its own. */
  fallbackDuration?: number;
  /** The dock tab whose minimum size applies (`useTranscriptDock`). */
  dockTab: DockTab;
  /** C has something to show. */
  hasCaptions: boolean;
  /** On screen while another pane plays this recording: Space calls
   *  `onClaim`, Escape still leaves fullscreen, every other key waits. */
  elsewhere?: boolean;
  /** Take playback over here, from `at` if given — Space, or a dock seek,
   *  while `elsewhere`. */
  onClaim?: (at?: number) => void;
  /** T. Without it, T shows or hides the dock. */
  onToggleDock?: () => void;
}

/** The player's state and actions; pass the result to `MediaPlayer`. */
export function useMediaPlayer({
  cues,
  resetKey,
  fallbackDuration = 0,
  dockTab,
  hasCaptions,
  elsewhere = false,
  onClaim,
  onToggleDock,
}: MediaPlayerOptions) {
  const cueStarts = useMemo(() => cues.map((cue) => cue.start), [cues]);
  const [activeCueIdx, setActiveCueIdx] = useState(-1);
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  /** The playhead for consumers that must not re-render with it: a prop would
   *  break a memoised dock on every `timeupdate`. */
  const atRef = useRef(0);
  /** The file's own length — a catalogue duration can run short of the
   *  recording. `fallbackDuration` stands in until metadata arrives. */
  const [fileDuration, setFileDuration] = useState(0);
  const [isFullscreen, setIsFullscreen] = useState(false);
  /** The element's own failure; callers show theirs beside it. */
  const [error, setError] = useState<string | null>(null);
  /** The dock's list is tracking playback. Shared by every list the dock
   *  shows, which are never mounted together. */
  const [following, setFollowing] = useState(true);
  /** Controls are over the frame, so they fade out of the way while playing. */
  const [controlsVisible, setControlsVisible] = useState(true);

  const speed = usePlayerPrefs((s) => s.speed);
  /** Only the focused pane's player answers keys. `tabId` is the pane. */
  const tabId = useTabId();
  const focused = useActivePaneId() === tabId;
  const volume = usePlayerPrefs((s) => s.volume);
  const muted = usePlayerPrefs((s) => s.muted);
  const captionsEnabled = usePlayerPrefs((s) => s.captionsEnabled);
  const dockOpen = usePlayerPrefs((s) => s.transcriptVisible);
  const setPrefs = usePlayerPrefs((s) => s.set);
  const closeDock = useCallback(() => setPrefs({ transcriptVisible: false }), [setPrefs]);

  /** The element while this player has it (`attach`). */
  const videoRef = useRef<HTMLVideoElement | null>(null);
  /** The element in state too, so the pref effects re-apply when a lecture's
   *  source switch hands the audio to another decoder. */
  const [leaderEl, setLeaderEl] = useState<HTMLVideoElement | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const videoAreaRef = useRef<HTMLDivElement>(null);
  const togglePlayRef = useRef<() => void>(() => {});
  // The key handler registers once and reaches actions through refs.
  const toggleDockRef = useRef<() => void>(() => {});
  const toggleCaptionsRef = useRef<() => void>(() => {});
  const elsewhereRef = useRef(elsewhere);
  elsewhereRef.current = elsewhere;
  const claimRef = useRef(onClaim);
  claimRef.current = onClaim;
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

  const dock = useTranscriptDock(containerRef, dockTab);

  // Reset per recording; where playback resumes is the caller's call.
  useEffect(() => {
    setActiveCueIdx(-1);
    setCurrentTime(0);
    setFileDuration(0);
    setIsPlaying(false);
    setError(null);
    setFollowing(true);
    followingRef.current = true;
  }, [resetKey]);

  /** What every clock in the player counts against. */
  const duration = fileDuration || fallbackDuration;

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

  /** A popover on the bar opened or closed: an open one pins the bar. */
  const onPanelOpenChange = useCallback(
    (open: boolean) => {
      panelOpenRef.current = open;
      revealControls();
    },
    [revealControls],
  );

  /** The pointer and the volume drag's holds on the bar, for the view. */
  const holds = useMemo(
    () => ({
      leaveArea: () => {
        pointerOnControlsRef.current = false;
        if (controlsHeld()) return;
        if (videoRef.current && !videoRef.current.paused) {
          setControlsVisible(false);
        }
      },
      enterBar: () => {
        pointerOnControlsRef.current = true;
        revealControls();
      },
      leaveBar: () => {
        pointerOnControlsRef.current = false;
        revealControls();
      },
      volumeDragging: (dragging: boolean) => {
        volumeDraggingRef.current = dragging;
        revealControls();
      },
    }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [revealControls],
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

  const toggleFullscreen = useCallback(async () => {
    const next = !isFullscreenRef.current;
    setIsFullscreen(next);
    if (!next) return;
    try {
      const win = getCurrentWindow();
      if (!(await win.isFullscreen())) await win.setFullscreen(true);
    } catch {
      /* not fatal — the overlay just covers a windowed app */
    }
  }, []);

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

  // The focused pane's player only. While `elsewhere`, Space plays here and
  // Escape still leaves fullscreen; every other key waits for ownership.
  useEffect(() => {
    if (!focused) return;
    const handler = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const tag = target?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      // An open popover (speed, layout, source) owns its arrows and space bar.
      if (target?.closest('[data-slot="popover-content"]')) return;

      if (elsewhereRef.current && e.key !== "Escape") {
        if (e.key === " ") {
          e.preventDefault();
          claimRef.current?.();
        }
        return;
      }

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
            toggleFullscreen();
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
            toggleDockRef.current();
          }
          break;
        case "Escape":
          if (isFullscreenRef.current) {
            e.preventDefault();
            toggleFullscreen();
          }
          break;
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [toggleFullscreen, focused]);

  const toggleDock = () => {
    if (onToggleDock) onToggleDock();
    else setPrefs({ transcriptVisible: !dockOpen });
  };
  toggleDockRef.current = toggleDock;

  // Captions come from the cues: no transcript, nothing to toggle.
  const toggleCaptions = () => {
    if (!hasCaptions) return;
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

    setActiveCueIdx(spanAt(cueStarts, t));
  };

  // A stream still being sized reports Infinity or NaN; keep the fallback.
  const handleLoadedMetadata = () => {
    const v = videoRef.current;
    if (!v) return;
    if (Number.isFinite(v.duration) && v.duration > 0) setFileDuration(v.duration);
  };

  // ── Follow ───────────────────────────────────────────────────────────────

  // `FollowList` does the scrolling; the player owns the flag and the ways back
  // to live, shared by every list in the dock.
  const onScrollAway = useCallback(() => {
    if (!followingRef.current) return;
    followingRef.current = false;
    setFollowing(false);
  }, []);

  const onBackToLive = useCallback(() => {
    if (followingRef.current) return;
    followingRef.current = true;
    setFollowing(true);
  }, []);

  // A seek from the dock while the other pane has the video takes it over.
  const seek = useCallback((seconds: number) => {
    if (videoRef.current) videoRef.current.currentTime = seconds;
    else if (elsewhereRef.current) claimRef.current?.(seconds);
  }, []);

  /** The scrub bar's seek: the clock moves before the element reports it. */
  const scrub = (seconds: number) => {
    setCurrentTime(seconds);
    if (videoRef.current) videoRef.current.currentTime = seconds;
  };

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

  // ── The element ──────────────────────────────────────────────────────────

  // The element may outlive this component (a lecture's does), so it is
  // attached, not rendered; listeners reach the current handlers through a ref.
  const mediaRef = useRef({
    timeUpdate: handleTimeUpdate,
    loadedMetadata: handleLoadedMetadata,
    togglePlay,
    backToLive: onBackToLive,
  });
  mediaRef.current = {
    timeUpdate: handleTimeUpdate,
    loadedMetadata: handleLoadedMetadata,
    togglePlay,
    backToLive: onBackToLive,
  };

  /**
   * Play `v` in this player: state follows it from now on. Returns the detach
   * for the caller's effect cleanup. `null` says the player has none — a
   * lecture's elements are with another pane, or not yet loaded.
   */
  const attach = useCallback((v: HTMLVideoElement | null): (() => void) => {
    videoRef.current = v;
    setLeaderEl(v);
    if (!v) return () => {};

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
    // A source that loads after one that failed (a re-download) clears it.
    const onLoadedData = () => setError(null);
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
    v.addEventListener("loadeddata", onLoadedData);

    return () => {
      v.removeEventListener("timeupdate", onTimeUpdate);
      v.removeEventListener("loadedmetadata", onLoadedMetadata);
      v.removeEventListener("click", onClick);
      v.removeEventListener("play", onPlay);
      v.removeEventListener("pause", onPause);
      v.removeEventListener("ended", onEnded);
      v.removeEventListener("error", onError);
      v.removeEventListener("loadeddata", onLoadedData);
      videoRef.current = null;
    };
  }, []);

  return {
    cues,
    /** The element while this player has it, for a caller's own listeners. */
    element: leaderEl,
    activeCueIdx,
    isPlaying,
    currentTime,
    atRef,
    duration,
    isFullscreen,
    error,
    following,
    controlsVisible,
    elsewhere,
    hasCaptions,
    captionsEnabled,
    dockOpen,
    dock,
    containerRef,
    videoAreaRef,
    attach,
    togglePlay,
    seek,
    scrub,
    toggleFullscreen,
    toggleDock,
    toggleCaptions,
    closeDock,
    onScrollAway,
    onBackToLive,
    revealControls,
    onPanelOpenChange,
    holds,
  };
}

export type MediaPlayerState = ReturnType<typeof useMediaPlayer>;

interface MediaPlayerProps {
  player: MediaPlayerState;
  /** The picture: frames an element is moved into, a `<video>` of the
   *  caller's own, or what shows before there is one. */
  children: ReactNode;
  /** Over the picture and the (then inert) control bar — a lecture's
   *  "Playing in the other pane" still. */
  overlay?: ReactNode;
  /** The dock panel, usually a `MediaDock`. */
  dock: ReactNode;
  /** Can play: a source is loaded. */
  canPlay: boolean;
  /** Streams the scrub bar's hover thumbnail. */
  previewSrc: string | null;
  /** Chapter boundaries notched into the scrub bar. */
  chapters?: number[];
  /** Second the lecture's content ends, flagged on the scrub bar. */
  endAt?: number | null;
  /** Above the scrub bar — the lecture's chapter name. */
  aboveSeek?: ReactNode;
  /** Buttons between speed and the dock button. */
  controls?: ReactNode;
  /** The dock button's label. */
  dockLabel: string;
  /** The caller's error, shown in place of the element's. */
  error?: string | null;
}

/** The player's frame: picture, captions, control bar, dock. */
export function MediaPlayer({
  player,
  children,
  overlay,
  dock: dockPanel,
  canPlay,
  previewSrc,
  chapters = NO_STARTS,
  endAt = null,
  aboveSeek,
  controls,
  dockLabel,
  error,
}: MediaPlayerProps) {
  const {
    cues,
    activeCueIdx,
    isPlaying,
    currentTime,
    duration,
    isFullscreen,
    controlsVisible,
    elsewhere,
    hasCaptions,
    captionsEnabled,
    dockOpen,
    dock: { dock, height, width, dropTarget, startResize },
    containerRef,
    videoAreaRef,
    holds,
  } = player;
  const volume = usePlayerPrefs((s) => s.volume);
  const muted = usePlayerPrefs((s) => s.muted);
  const speed = usePlayerPrefs((s) => s.speed);
  const setPrefs = usePlayerPrefs((s) => s.set);
  const shownError = error ?? player.error;

  return (
    // The dock side is a flex direction: `*-reverse` puts the panel before the
    // video stack visually while leaving the divider between the two.
    <div
      ref={containerRef}
      className={cn(
        "flex overflow-hidden min-h-0 min-w-0 bg-background relative",
        // Over the sidebar and the tab strip alike.
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
            !controlsVisible && !elsewhere && "cursor-none",
          )}
          onPointerMove={player.revealControls}
          onPointerLeave={holds.leaveArea}
        >
          {children}
          {overlay}
          {captionsEnabled && activeCueIdx >= 0 && !elsewhere && (
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
              "absolute inset-x-0 bottom-0 z-30 transition-opacity will-change-[opacity] duration-200",
              controlsVisible ? "opacity-100" : "opacity-0 pointer-events-none",
            )}
            inert={elsewhere}
            onPointerEnter={holds.enterBar}
            onPointerLeave={holds.leaveBar}
          >
            <div className="pointer-events-none absolute inset-x-0 bottom-0 h-24 bg-gradient-to-t from-black/85 via-black/45 to-transparent" />

            <div className="relative px-3 pb-1">
              {aboveSeek}

              <SeekBar
                value={Math.min(currentTime, duration)}
                max={duration || 1}
                previewSrc={previewSrc}
                chapters={chapters}
                endAt={endAt}
                onSeek={player.scrub}
              />

              <div className="flex h-9 items-center gap-0.5">
                <ControlButton
                  label={isPlaying ? "Pause" : "Play"}
                  onClick={player.togglePlay}
                  disabled={!canPlay}
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
                  onDraggingChange={holds.volumeDragging}
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
                  onOpenChange={player.onPanelOpenChange}
                />

                {controls}

                {/* Labelled by the tab in front: it folds the whole dock. */}
                <ControlButton
                  label={dockLabel}
                  active={dockOpen}
                  onClick={player.toggleDock}
                >
                  <SidebarSimple
                    size={16}
                    className={cn("transition-transform", DOCK_ICON_FACING[dock])}
                  />
                </ControlButton>

                <ControlButton
                  label={
                    captionsEnabled ? "Hide captions (C)" : "Show captions (C)"
                  }
                  active={captionsEnabled}
                  disabled={!hasCaptions}
                  onClick={player.toggleCaptions}
                >
                  <ClosedCaptioning
                    size={17}
                    weight={captionsEnabled ? "fill" : "regular"}
                  />
                </ControlButton>

                <ControlButton
                  label={isFullscreen ? "Exit fullscreen (F)" : "Fullscreen (F)"}
                  onClick={player.toggleFullscreen}
                >
                  {isFullscreen ? <ArrowsIn size={16} /> : <ArrowsOut size={16} />}
                </ControlButton>
              </div>
            </div>
          </div>
        </div>

        {shownError && (
          <div className="shrink-0 px-3 py-1.5 text-[11px] text-destructive border-t border-border bg-destructive/5 break-words">
            {shownError}
          </div>
        )}
      </div>

      {dockOpen && <DockResizeHandle dock={dock} onPointerDown={startResize} />}
      {dockPanel}

      {dropTarget && (
        <DockDropPreview dock={dropTarget} height={height} width={width} />
      )}
    </div>
  );
}
