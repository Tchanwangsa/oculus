import { leaderVideo, els, onScreen, parked, state } from "./source";
import type { SourceNum } from "@/lib/db";

/** Drift (seconds) a follower is left alone at — correcting less would
 *  stutter the picture for no visible gain. */
export const MAX_DRIFT = 0.35;

/** Drift worth a seek. Between this and `MAX_DRIFT` the follower's rate is
 *  trimmed instead, since a seek flushes the decoder and stutters. */
const SEEK_DRIFT = 1.5;

/** Fraction a trimmed follower's rate is nudged by. */
const RATE_TRIM = 0.08;

/** Both files are trimmed alike (`echo360::TRIM_SECS`), so in sync means the
 *  same `currentTime` — no offset. */
export function syncFollowers() {
  const lead = leaderVideo();
  if (!lead) return;
  for (const source of onScreen) {
    if (source === state.leaderSource) continue;
    const f = els.get(source);
    if (!f) continue;
    f.muted = true;
    // readyState 0 has no timeline to seek on yet; the next tick catches it.
    if (f.readyState > 0) alignFollower(f, lead);
    if (lead.paused) {
      if (!f.paused) f.pause();
    } else if (f.paused) {
      f.play().catch(() => {
        /* a follower that won't start is not worth failing the lecture over */
      });
    }
  }
}

function alignFollower(f: HTMLVideoElement, lead: HTMLVideoElement) {
  const rate = lead.playbackRate;
  const behind = lead.currentTime - f.currentTime;
  const off = Math.abs(behind);

  if (lead.paused || off > SEEK_DRIFT) {
    if (off > MAX_DRIFT) f.currentTime = lead.currentTime;
    if (f.playbackRate !== rate) f.playbackRate = rate;
    return;
  }
  if (off > MAX_DRIFT) {
    const trimmed = Number((rate * (behind > 0 ? 1 + RATE_TRIM : 1 - RATE_TRIM)).toFixed(3));
    if (f.playbackRate !== trimmed) f.playbackRate = trimmed;
    return;
  }
  if (f.playbackRate !== rate) f.playbackRate = rate;
}

/** Seek an element to `at` — now if it can, otherwise the moment it can.
 *  Null leaves the position alone. */
export function joinAt(v: HTMLVideoElement, at: number | null, play: boolean) {
  const go = () => {
    if (at != null && Number.isFinite(at) && at >= 0) {
      try {
        v.currentTime = at;
      } catch {
        /* the element rejected the seek; playback simply starts at zero */
      }
    }
    if (play) v.play().catch(() => {});
  };
  if (v.readyState >= 1) go();
  else v.addEventListener("loadedmetadata", go, { once: true });
}

/** Take a source off screen: paused, parked, but still loaded for a fast return. */
export function release(source: SourceNum) {
  onScreen.delete(source);
  const v = els.get(source);
  if (!v) return;
  v.pause();
  parked().appendChild(v);
}

// WebKit stops feeding a hidden page's video frames, so followers come back
// out of sync; realign on return.
document.addEventListener("visibilitychange", () => {
  if (document.hidden) return;
  if (leaderVideo()) syncFollowers();
});
