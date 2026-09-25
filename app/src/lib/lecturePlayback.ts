import {
  markLectureComplete,
  updateLectureProgress,
  type Lecture,
  type SourceNum,
} from "@/lib/db";

/**
 * The app's `<video>` elements and progress writes, owned here rather than by
 * the player so a route change (tab switch, expanding the peek) hands them to
 * the next host without interrupting playback.
 *
 * One element per Echo360 source (`docs/sync.md`). Exactly one is the
 * **leader** — audio, clock, progress writes; the others follow it, muted.
 */

/** Fired after a progress write so a mounted list can refresh its rows. */
export const LECTURE_PROGRESS_EVENT = "oculus-lecture-progress";

/** How often a playing lecture writes down where it is. */
const SAVE_EVERY_MS = 5000;

/** How often a follower is nudged back onto the leader's clock. */
const SYNC_EVERY_MS = 1000;

/** Drift (seconds) a follower is left alone at — correcting less would
 *  stutter the picture for no visible gain. */
const MAX_DRIFT = 0.35;

/** Drift worth a seek. Between this and `MAX_DRIFT` the follower's rate is
 *  trimmed instead, since a seek flushes the decoder and stutters. */
const SEEK_DRIFT = 1.5;

/** Fraction a trimmed follower's rate is nudged by. */
const RATE_TRIM = 0.08;

/** Seconds from the end that count as watched. */
const COMPLETE_WITHIN = 30;

/** Which player has the elements: the lecture page, or the peek beside a page. */
export type PlaybackHost = "page" | "panel";

export interface PlaybackOwner {
  tab: number;
  host: PlaybackHost;
}

export interface SourcePlan {
  source: SourceNum;
  /** Media-server URL for that source's file. */
  src: string;
  /** The element in the player's frame the video is moved into. */
  host: HTMLElement;
}

const els = new Map<SourceNum, HTMLVideoElement>();
/** Sources currently on screen. The first of them is `leaderSource`. */
const onScreen = new Set<SourceNum>();
let leaderSource: SourceNum | null = null;
let parkedHost: HTMLDivElement | null = null;
let saveTicker: ReturnType<typeof setInterval> | null = null;
let syncTicker: ReturnType<typeof setInterval> | null = null;
let current: Lecture | null = null;
/** Last second written, so a paused element doesn't rewrite the same row. */
let lastWritten = -1;
/** The player playback belongs to (see `ownsPlayback`). */
let ownerTab: number | null = null;
let ownerHost: PlaybackHost = "page";
/**
 * Bumped on every claim. Two players can be mounted at once and the new one
 * adopts the elements before the old one unmounts; the token stops the old
 * one parking them after the handover.
 */
let claim = 0;

/** Off screen rather than `display: none`, where a browser may stop media. */
function parked(): HTMLDivElement {
  if (!parkedHost) {
    const host = document.createElement("div");
    host.setAttribute("aria-hidden", "true");
    // A real size, not 1×1: WebKit sizes the decode path to the picture, so a
    // tiny host brings the video back blurry until the decoder catches up.
    host.style.cssText =
      "position:fixed;left:-10000px;top:0;width:480px;height:270px;overflow:hidden;pointer-events:none";
    document.body.appendChild(host);
    parkedHost = host;
  }
  return parkedHost;
}

function leaderVideo(): HTMLVideoElement | null {
  return leaderSource != null ? (els.get(leaderSource) ?? null) : null;
}

/** The element for one source, if it has ever been on screen. */
export function videoForSource(source: SourceNum): HTMLVideoElement | null {
  return els.get(source) ?? null;
}

function videoFor(source: SourceNum): HTMLVideoElement {
  const existing = els.get(source);
  if (existing) return existing;
  const v = document.createElement("video");
  v.playsInline = true;
  v.preload = "metadata";
  v.className = "w-full h-full object-contain";
  v.dataset.lectureSource = String(source);
  parked().appendChild(v);
  els.set(source, v);
  return v;
}

// ── Leader ───────────────────────────────────────────────────────────────────

// Pause/end/seek write immediately and re-align followers, which cannot infer
// them from their own clock.
const onLeaderPlay = () => {
  startTickers();
  syncFollowers();
};
const onLeaderPause = () => {
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

function setLeader(next: SourceNum | null) {
  if (leaderSource === next) return;
  const previous = leaderVideo();
  if (previous) {
    for (const [type, fn] of LEADER_EVENTS) previous.removeEventListener(type, fn);
    // Now, not on the next sync tick: two audio tracks for a beat is audible.
    previous.muted = true;
  }
  leaderSource = next;
  const v = leaderVideo();
  if (v) for (const [type, fn] of LEADER_EVENTS) v.addEventListener(type, fn);
  if (v && !v.paused) startTickers();
}

function startTickers() {
  saveTicker ??= setInterval(() => void saveLectureProgress(), SAVE_EVERY_MS);
  if (!syncTicker && onScreen.size > 1) {
    syncTicker = setInterval(syncFollowers, SYNC_EVERY_MS);
  }
}

function stopTickers() {
  if (saveTicker) clearInterval(saveTicker);
  if (syncTicker) clearInterval(syncTicker);
  saveTicker = null;
  syncTicker = null;
}

// ── Follower sync ────────────────────────────────────────────────────────────

/** Both files are trimmed alike (`echo360::TRIM_SECS`), so in sync means the
 *  same `currentTime` — no offset. */
function syncFollowers() {
  const lead = leaderVideo();
  if (!lead) return;
  for (const source of onScreen) {
    if (source === leaderSource) continue;
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

/** Seek an element to `at` — now if it can, otherwise the moment it can. */
function joinAt(v: HTMLVideoElement, at: number, play: boolean) {
  const go = () => {
    if (Number.isFinite(at) && at > 0) {
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
function release(source: SourceNum) {
  onScreen.delete(source);
  const v = els.get(source);
  if (!v) return;
  v.pause();
  parked().appendChild(v);
}

// ── The one call the player makes ────────────────────────────────────────────

/**
 * Reconcile the elements to `lecture` laid out as `plan`, whose **first entry
 * is the leader**. Called by the on-screen player whenever what it wants
 * changes. A changed leader is a different decoder, so position and play state
 * are carried across by hand. Returns the leader element.
 */
export function syncLectureSources(
  lecture: Lecture,
  plan: SourcePlan[],
  owner: PlaybackOwner,
): HTMLVideoElement | null {
  ownerTab = owner.tab;
  ownerHost = owner.host;
  claim++;

  const lectureChanged = current?.id !== lecture.id;
  if (lectureChanged && current) void saveLectureProgress();
  current = lecture;
  if (lectureChanged) lastWritten = -1;

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
  syncFollowers();
  if (lead && !lead.paused) startTickers();
  return lead;
}

/** The current claim, for a player to pass back to `parkLectureVideos`. */
export function playbackClaim(): number {
  return claim;
}

export function isLecturePlaying(): boolean {
  const v = leaderVideo();
  return !!v && !v.paused && !v.ended;
}

export function playingLecture(): Lecture | null {
  return isLecturePlaying() ? current : null;
}

/**
 * Whether playback belongs to this tab (and, given `host`, to that player).
 * Omit the host for closing the tab; pass `"page"` for navigation within it,
 * which a peek survives.
 */
export function ownsPlayback(tabId: number, host?: PlaybackHost): boolean {
  return ownerTab === tabId && (host == null || ownerHost === host);
}

/**
 * Park every element off screen, still playing if it was. With `forClaim`, a
 * no-op once another player has claimed them.
 */
export function parkLectureVideos(forClaim?: number) {
  if (forClaim != null && forClaim !== claim) return;
  if (!leaderVideo()) return;
  if (!isLecturePlaying()) void saveLectureProgress();
  for (const v of els.values()) parked().appendChild(v);
}

/** Stop for good: the lecture was closed, not navigated away from. */
export function stopLecturePlayback() {
  if (!els.size) return;
  for (const v of els.values()) v.pause();
  stopTickers();
  void saveLectureProgress();
  parkLectureVideos();
  ownerTab = null;
  ownerHost = "page";
}

async function saveLectureProgress(): Promise<void> {
  const v = leaderVideo();
  const lecture = current;
  if (!v || !lecture) return;
  const seconds = Math.floor(v.currentTime);
  if (seconds === lastWritten) return;
  lastWritten = seconds;
  try {
    await updateLectureProgress(lecture.id, seconds);
    // The file's length beats Echo360's catalogue duration, which runs short.
    const total =
      Number.isFinite(v.duration) && v.duration > 0
        ? v.duration
        : lecture.duration_seconds;
    if (total > 0 && v.currentTime >= total - COMPLETE_WITHIN) {
      await markLectureComplete(lecture.id);
    }
    window.dispatchEvent(
      new CustomEvent(LECTURE_PROGRESS_EVENT, {
        detail: { id: lecture.id, seconds },
      }),
    );
  } catch {
    /* a lost position is not worth an error in the player */
  }
}

// WebKit stops feeding a hidden page's video frames, so followers come back
// out of sync; realign on return.
document.addEventListener("visibilitychange", () => {
  if (document.hidden) return;
  if (leaderVideo()) syncFollowers();
});

// Best effort: the write is async and the webview is going away.
window.addEventListener("pagehide", () => void saveLectureProgress());
