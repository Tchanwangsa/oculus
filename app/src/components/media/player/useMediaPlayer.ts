import { useCallback, useEffect, useRef, useState } from "react";
import type { Cue } from "@/lib/lectures/media";
import { useActivePaneId } from "@/stores/shell/tabStore";
import { useTabId } from "@/components/tabs/TabContext";
import { useTranscriptDock, type DockSide } from "@/hooks/lectures/useTranscriptDock";
import { usePlayerPrefs, type DockTab } from "@/stores/lectures/playerPrefsStore";
import { useControlsVisibility } from "@/components/media/player/useControlsVisibility";
import { useFollow } from "@/components/media/player/useFollow";
import { useFullscreen } from "@/components/media/player/useFullscreen";
import { useKeyboardShortcuts } from "@/components/media/player/useKeyboardShortcuts";
import { useMediaElement } from "@/components/media/player/useMediaElement";
import { usePlaybackEvents } from "@/components/media/player/usePlaybackEvents";

export interface MediaPlayerOptions {
  cues: Cue[];
  /** Changes when the player shows another recording: the clock, play state,
   *  follow and the element's error start over. */
  resetKey: string;
  /** The recording's length until the file reports its own. */
  fallbackDuration?: number;
  /** The dock tab whose minimum size applies (`useTranscriptDock`). */
  dockTab: DockTab;
  /** Width the video keeps beside a side dock, which draws at the bottom
   *  when the player is narrower (`useTranscriptDock`'s default if absent). */
  dockKeepWidth?: number;
  /** The dock's side, when the caller keeps it rather than the shared prefs. */
  dockSide?: DockSide;
  /** Side docks draw at the bottom while false; fullscreen keeps the saved side. */
  allowSideDock?: boolean;
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
  dockKeepWidth,
  dockSide,
  allowSideDock = true,
  hasCaptions,
  elsewhere = false,
  onClaim,
  onToggleDock,
}: MediaPlayerOptions) {
  /** The element while this player has it (`attach`). */
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const {
    activeCueIdx,
    setActiveCueIdx,
    currentTime,
    setCurrentTime,
    atRef,
    fileDuration,
    setFileDuration,
    handleTimeUpdate,
    handleLoadedMetadata,
  } = usePlaybackEvents(cues, videoRef);
  const [isPlaying, setIsPlaying] = useState(false);
  /** The element's own failure; callers show theirs beside it. */
  const [error, setError] = useState<string | null>(null);
  const { following, setFollowing, followingRef, onScrollAway, onBackToLive } =
    useFollow();

  /** Only the focused pane's player answers keys. `tabId` is the pane. */
  const tabId = useTabId();
  const focused = useActivePaneId() === tabId;
  const captionsEnabled = usePlayerPrefs((s) => s.captionsEnabled);
  const dockOpen = usePlayerPrefs((s) => s.transcriptVisible);
  const setPrefs = usePlayerPrefs((s) => s.set);
  const closeDock = useCallback(() => setPrefs({ transcriptVisible: false }), [setPrefs]);

  const containerRef = useRef<HTMLDivElement>(null);
  const videoAreaRef = useRef<HTMLDivElement>(null);
  const togglePlayRef = useRef<() => void>(() => {});
  const toggleDockRef = useRef<() => void>(() => {});
  const toggleCaptionsRef = useRef<() => void>(() => {});
  const elsewhereRef = useRef(elsewhere);
  elsewhereRef.current = elsewhere;
  const claimRef = useRef(onClaim);
  claimRef.current = onClaim;

  const { isFullscreen, isFullscreenRef, toggleFullscreen } = useFullscreen();
  const dock = useTranscriptDock(
    containerRef, dockTab, dockKeepWidth, dockSide, allowSideDock || isFullscreen,
  );

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

  const { controlsVisible, revealControls, onPanelOpenChange, holds } =
    useControlsVisibility(isPlaying, videoRef);
  useKeyboardShortcuts({
    focused,
    toggleFullscreen,
    isFullscreenRef,
    videoRef,
    elsewhereRef,
    claimRef,
    togglePlayRef,
    toggleDockRef,
    toggleCaptionsRef,
  });

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

  const { leaderEl, attach } = useMediaElement({
    videoRef,
    handlers: {
      timeUpdate: handleTimeUpdate,
      loadedMetadata: handleLoadedMetadata,
      togglePlay,
      backToLive: onBackToLive,
    },
    setIsPlaying,
    setFileDuration,
    setError,
  });

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
