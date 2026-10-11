import { markPlaying } from "@/lib/lectures/playbackOwner";
import { pauseLibraryVideos } from "@/lib/lectures/media";
import type { SourceNum } from "@/lib/db";
import { syncFollowers } from "./follower";
import { saveLectureProgress } from "./progress";
import { leaderVideo, onScreen, state } from "./source";

/** How often a playing lecture writes down where it is. */
const SAVE_EVERY_MS = 5000;

/** How often a follower is nudged back onto the leader's clock. */
const SYNC_EVERY_MS = 1000;

// Pause/end/seek write immediately and re-align followers, which cannot infer
// them from their own clock.
const onLeaderPlay = () => {
  // One sound at a time: a library video playing in another pane stops.
  pauseLibraryVideos();
  markPlaying(true);
  startTickers();
  syncFollowers();
};
const onLeaderPause = () => {
  markPlaying(false);
  stopTickers();
  void saveLectureProgress();
  syncFollowers();
};
const onLeaderEnded = onLeaderPause;
const onLeaderSeeked = () => {
  void saveLectureProgress();
  syncFollowers();
};
const onLeaderRateChange = () => syncFollowers();

const LEADER_EVENTS: [string, EventListener][] = [
  ["play", onLeaderPlay],
  ["pause", onLeaderPause],
  ["ended", onLeaderEnded],
  ["seeked", onLeaderSeeked],
  ["ratechange", onLeaderRateChange],
];

export function setLeader(next: SourceNum | null) {
  if (state.leaderSource === next) return;
  const previous = leaderVideo();
  if (previous) {
    for (const [type, fn] of LEADER_EVENTS) previous.removeEventListener(type, fn);
    // Now, not on the next sync tick: two audio tracks for a beat is audible.
    previous.muted = true;
  }
  state.leaderSource = next;
  const v = leaderVideo();
  if (v) for (const [type, fn] of LEADER_EVENTS) v.addEventListener(type, fn);
  if (v && !v.paused) startTickers();
}

export function startTickers() {
  state.saveTicker ??= setInterval(() => void saveLectureProgress(), SAVE_EVERY_MS);
  if (!state.syncTicker && onScreen.size > 1) {
    state.syncTicker = setInterval(syncFollowers, SYNC_EVERY_MS);
  }
}

export function stopTickers() {
  if (state.saveTicker) clearInterval(state.saveTicker);
  if (state.syncTicker) clearInterval(state.syncTicker);
  state.saveTicker = null;
  state.syncTicker = null;
}
