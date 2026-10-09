import {
  clearPlaybackOwner,
  markPlaying,
  playbackOwner,
  releasePlayback,
} from "@/lib/lectures/playbackOwner";
import type { Lecture } from "@/lib/db";
import { stopTickers } from "./leader";
import { saveLectureProgress } from "./progress";
import { els, leaderVideo, parked, state } from "./source";

/** The current claim, for a player to pass back to `parkLectureVideos`. */
export function playbackClaim(): number {
  return state.claim;
}

export function isLecturePlaying(): boolean {
  const v = leaderVideo();
  return !!v && !v.paused && !v.ended;
}

export function playingLecture(): Lecture | null {
  return isLecturePlaying() ? state.current : null;
}

/** Whether playback belongs to the player in this pane. */
export function ownsPlayback(paneId: number): boolean {
  return playbackOwner().pane === paneId;
}

/**
 * Park every element off screen, still playing if it was. With `forClaim`, a
 * no-op once another player has adopted them.
 */
export function parkLectureVideos(forClaim?: number) {
  if (forClaim != null && forClaim !== state.claim) return;
  if (!leaderVideo()) return;
  if (!isLecturePlaying()) void saveLectureProgress();
  for (const v of els.values()) parked().appendChild(v);
}

/** The player in `paneId` unmounted; a paused owner gives playback up
 *  (`releasePlayback`). */
export function releaseLecturePlayer(paneId: number) {
  markPlaying(isLecturePlaying());
  releasePlayback(paneId);
}

/** Pause the lecture, wherever its elements are — a library video started.
 *  Followers follow through the leader's `pause`. */
export function pauseLecturePlayback() {
  const v = leaderVideo();
  if (v && !v.paused) v.pause();
}

/** Stop for good: the lecture was closed, not navigated away from. */
export function stopLecturePlayback() {
  clearPlaybackOwner();
  if (!els.size) return;
  for (const v of els.values()) v.pause();
  stopTickers();
  void saveLectureProgress();
  parkLectureVideos();
}
