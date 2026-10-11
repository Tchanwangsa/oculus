import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { usePlayerPrefs } from "@/stores/lectures/playerPrefsStore";
import { panelOnScreen } from "@/components/media/player/constants";

interface MediaElementHandlers {
  timeUpdate: () => void;
  loadedMetadata: () => void;
  togglePlay: () => void;
  backToLive: () => void;
}

interface MediaElementArgs {
  videoRef: RefObject<HTMLVideoElement | null>;
  handlers: MediaElementHandlers;
  setIsPlaying: (playing: boolean) => void;
  setFileDuration: (seconds: number) => void;
  setError: (message: string | null) => void;
}

/** The element this player plays: attaching it, the listeners that feed the
 *  player's state, and the speed and volume prefs applied to it. */
export function useMediaElement({
  videoRef,
  handlers,
  setIsPlaying,
  setFileDuration,
  setError,
}: MediaElementArgs) {
  const speed = usePlayerPrefs((s) => s.speed);
  const volume = usePlayerPrefs((s) => s.volume);
  const muted = usePlayerPrefs((s) => s.muted);

  /** The element in state too, so the pref effects re-apply when a lecture's
   *  source switch hands the audio to another decoder. */
  const [leaderEl, setLeaderEl] = useState<HTMLVideoElement | null>(null);

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

  // The element may outlive this component (a lecture's does), so it is
  // attached, not rendered; listeners reach the current handlers through a ref.
  const mediaRef = useRef(handlers);
  mediaRef.current = handlers;

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

  return { leaderEl, attach };
}
