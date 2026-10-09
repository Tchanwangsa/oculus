import { markAdopted, markPlaying } from "@/lib/lectures/playbackOwner";
import type { Lecture } from "@/lib/db";
import { MAX_DRIFT, joinAt, release, syncFollowers } from "./follower";
import { setLeader, startTickers } from "./leader";
import { isLecturePlaying } from "./ownership";
import { saveLectureProgress } from "./progress";
import { leaderVideo, onScreen, state, videoFor, type SourcePlan } from "./source";

/**
 * Reconcile the elements to `lecture` laid out as `plan`, whose **first entry
 * is the leader**. Called by the player that may adopt them (`mayAdopt`)
 * whenever what it wants changes; its pane, `paneId` in strip tab `tabId`,
 * then owns playback. A changed leader is a different decoder, so position and
 * play state are carried across by hand. `start` plays the leader, from `at`
 * if given: a player taking playback over on a user action. Returns the
 * leader element.
 */
export function syncLectureSources(
  lecture: Lecture,
  plan: SourcePlan[],
  paneId: number,
  tabId: number,
  start?: { at?: number },
): HTMLVideoElement | null {
  state.claim++;

  const lectureChanged = state.current?.id !== lecture.id;
  if (lectureChanged && state.current) void saveLectureProgress();
  state.current = lecture;
  if (lectureChanged) state.lastWritten = -1;
  // A start asked for before the route changed is spent on the next lecture.
  let begin = start;
  if (lectureChanged && state.autoplay) {
    if (state.autoplay.id === lecture.id) begin ??= { at: state.autoplay.at };
    state.autoplay = null;
  }

  // Null for a fresh lecture, which starts from its saved progress.
  const outgoing = lectureChanged ? null : leaderVideo();
  const at = outgoing ? outgoing.currentTime : null;
  const wasPlaying = !!outgoing && !outgoing.paused && !outgoing.ended;

  const wanted = new Set(plan.map((p) => p.source));
  for (const source of [...onScreen]) if (!wanted.has(source)) release(source);

  let leaderIsFresh = false;
  for (const p of plan) {
    const v = videoFor(p.source);
    const fresh = v.dataset.lectureId !== lecture.id || v.getAttribute("src") !== p.src;
    if (p.source === plan[0].source) leaderIsFresh = fresh;
    if (fresh) {
      v.dataset.lectureId = lecture.id;
      v.src = p.src;
      joinAt(v, at ?? lecture.progress_seconds, wasPlaying && p.source === plan[0].source);
    }
    if (v.parentElement !== p.host) p.host.appendChild(v);
    onScreen.add(p.source);
  }

  setLeader(plan[0]?.source ?? null);
  const lead = leaderVideo();
  // An already-loaded leader taking over still needs moving; a fresh one got `at` above.
  if (lead && !leaderIsFresh && at != null && Math.abs(lead.currentTime - at) > MAX_DRIFT) {
    joinAt(lead, at, wasPlaying);
  }
  if (lead && begin) joinAt(lead, begin.at ?? null, true);
  syncFollowers();
  if (lead && !lead.paused) startTickers();
  markAdopted(paneId, tabId);
  markPlaying(isLecturePlaying());
  return lead;
}
