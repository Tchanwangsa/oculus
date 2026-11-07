import { leaderVideo, els, onScreen, parked, state } from "./source";
import type { SourceNum } from "@/lib/db";

/** Drift (seconds) a follower is left alone at — correcting less would
 *  stutter the picture for no visible gain. */
export const MAX_DRIFT = 0.35;

/** Drift past which the error is a jump, not seek latency, so nothing is learned from it. */
const JUMP_DRIFT = 1.5;

/** How long a landed seek takes to start playing again: until then its
 *  `currentTime` is the target, not where the picture is. */
const SETTLE_MS = 1500;

/** Ceiling on the learned seek latency, in wall seconds. */
const MAX_SEEK_LAG = 1;

/** Wall seconds a follower's seek takes to play again, learned from each
 *  landing; a correcting seek aims this far ahead. Never trim the rate instead:
 *  WebKit stalls a video on every `playbackRate` change (`docs/viewers.md`). */
let seekLag = 0.3;

/** A follower's last correcting seek: when, and whether its error teaches `seekLag`. */
const settling = new WeakMap<HTMLVideoElement, { at: number; learn: boolean }>();

/** The leader seeked: followers take its exact time, since both restart together. */
let leaderJumped = false;

/** Both files are trimmed alike (`echo360::trim_video`, by the university's
 *  lead-in), so in sync means the same `currentTime` — no offset. */
export function syncFollowers() {
  const lead = leaderVideo();
  if (!lead) return;
  for (const source of onScreen) {
    if (source === state.leaderSource) continue;
    const f = els.get(source);
    if (!f) continue;
    f.muted = true;
    // Before the readyState check: a fresh element's rate is reset by its load.
    if (f.playbackRate !== lead.playbackRate) f.playbackRate = lead.playbackRate;
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
  if (!lead.seeking) leaderJumped = false;
}

/** The leader's `seeked`: realign followers to it exactly. */
export function followLeaderSeek() {
  leaderJumped = true;
  syncFollowers();
}

function alignFollower(f: HTMLVideoElement, lead: HTMLVideoElement) {
  if (lead.seeking) return;
  const now = performance.now();
  if (leaderJumped || lead.paused) {
    if (leaderJumped || Math.abs(lead.currentTime - f.currentTime) > MAX_DRIFT) {
      f.currentTime = lead.currentTime;
      settling.set(f, { at: now, learn: false });
    }
    return;
  }
  if (f.seeking) return;

  const behind = lead.currentTime - f.currentTime;
  const off = Math.abs(behind);
  const last = settling.get(f);
  if (last && off <= JUMP_DRIFT) {
    if (now - last.at < SETTLE_MS) return;
    // Half the error: one landing is noisy, and a full step overshoots.
    if (last.learn) {
      seekLag = Math.min(MAX_SEEK_LAG, Math.max(0, seekLag + behind / lead.playbackRate / 2));
    }
  }
  settling.delete(f);
  if (off > MAX_DRIFT) {
    f.currentTime = lead.currentTime + seekLag * lead.playbackRate;
    settling.set(f, { at: now, learn: true });
  }
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
