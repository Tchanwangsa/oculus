import {
  markLectureComplete,
  updateLectureProgress,
  type Lecture,
  type SourceNum,
} from "@/lib/db";

/**
 * The app's `<video>` elements, owned here rather than by the player
 * component.
 *
 * A tab switch is a route change, and a route change unmounts the page —
 * which used to take the video element, and the lecture, with it. The elements
 * living out here instead means switching tabs (or expanding the peek into its
 * own tab, which is the same unmount) hands them from one host to the next
 * without interrupting playback: the lecture keeps going in the background and
 * picks up on screen exactly where it is when you come back.
 *
 * These are DOM nodes in the *visible* webview, not a hidden WebView — the
 * suspension problem that keeps scraping in Rust does not apply to them.
 *
 * There is one element **per source**, because an Echo360 capture is two
 * streams of the same hour (`docs/sync.md`) and the player can show both at
 * once. They are two files, so they are two elements and two decoders; what
 * makes them one lecture is the sync below. Exactly one is the **leader**: it
 * carries the audio, it is the clock every readout counts against, and it is
 * what writes progress. The others follow it, muted.
 *
 * Progress is written from here too, for the same reason as the elements:
 * the position has to keep being saved while no player is mounted to save it.
 */

/** Fired after a progress write so a mounted list can refresh its rows. */
export const LECTURE_PROGRESS_EVENT = "oculus-lecture-progress";

/** How often a playing lecture writes down where it is. */
const SAVE_EVERY_MS = 5000;

/** How often a follower is nudged back onto the leader's clock. */
const SYNC_EVERY_MS = 1000;

/**
 * Drift a follower is left alone at. A seek is a re-buffer, so correcting a
 * few frames of slip would stutter the picture to fix something nobody can
 * see; a third of a second is past the point where two views of one room stop
 * looking simultaneous.
 */
const MAX_DRIFT = 0.35;

/**
 * Drift worth a *seek*. Between this and `MAX_DRIFT` the follower is walked
 * back onto the clock with a trim to its playback rate instead.
 *
 * A seek flushes the decoder, and a follower that cannot quite hold real time
 * — which is what coming back to a backgrounded tab leaves you with — earns
 * one every tick. On a slide that is invisible; on the room camera it is a
 * picture that stutters once a second and never settles. Riding the rate
 * closes the same gap without ever dropping a frame.
 */
const SEEK_DRIFT = 1.5;

/** How hard a trimmed follower chases the leader: 8% off its rate, so a
 *  half-second gap closes over a few seconds, invisibly. */
const RATE_TRIM = 0.08;

/** Ending within this of the finish counts as watched. */
const COMPLETE_WITHIN = 30;

/** One frame of a lecture on screen: which stream, from where, in what box. */
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
/** The lecture the elements are loaded with, for the writes below. */
let current: Lecture | null = null;
/** Last second written, so a paused element doesn't rewrite the same row. */
let lastWritten = -1;
/**
 * The tab the player was mounted in when it took the elements. Playback
 * belongs to that tab: switching to another one leaves it alone, while closing
 * it or navigating it elsewhere is leaving the lecture, and asks first.
 */
let ownerTab: number | null = null;
/**
 * Which mounted player the elements belong to, as a counter bumped on every
 * claim.
 *
 * Two players can be mounted over the same lecture at once — the side panel is
 * shell furniture and every tab's pane stays mounted behind the one in front —
 * and handing a lecture from one to the other is a normal move: expanding the
 * panel into its own tab mounts the page's player before the panel's is taken
 * down. Without a token the one leaving parks the elements a beat *after* the
 * new one adopted them, which leaves the picture off screen and playing: black
 * frame, audio carrying on, controls counting up. A player parks only what is
 * still its own.
 */
let claim = 0;

/**
 * Where an element sits while it is not in a player. Off screen rather than
 * `display: none`: a hidden subtree is where a browser feels entitled to stop
 * a media element, and this host exists precisely so it doesn't stop.
 */
function parked(): HTMLDivElement {
  if (!parkedHost) {
    const host = document.createElement("div");
    host.setAttribute("aria-hidden", "true");
    // Off screen at a *real size*, not collapsed to a pixel. The elements are
    // `w-full h-full`, so a 1×1 host is a 1×1 picture, and WebKit sizes the
    // decode path to the picture it is asked for — park a playing lecture in
    // one and it comes back to the player as mush until the decoder catches
    // up. 480×270 costs nothing here (nothing off screen is painted) and
    // keeps the presentation size roughly what it was.
    host.style.cssText =
      "position:fixed;left:-10000px;top:0;width:480px;height:270px;overflow:hidden;pointer-events:none";
    document.body.appendChild(host);
    parkedHost = host;
  }
  return parkedHost;
}

/** The element carrying the audio and the clock. */
export function leaderVideo(): HTMLVideoElement | null {
  return leaderSource != null ? (els.get(leaderSource) ?? null) : null;
}

/** The element for one source, if it has ever been on screen. The player uses
 *  it to read the picture's own dimensions, which is what locks the PIP box's
 *  aspect ratio to the recording rather than to an assumed 16:9. */
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

// Playing writes every few seconds; the moments that end a stretch of playback
// write immediately, because they are exactly when the position stops changing
// on its own. Each of them also re-aligns the followers, since a pause or a
// seek is the one thing they cannot infer from their own clock.
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
    // Demoted, not stopped: it may still be on screen as a follower, and a
    // follower is silent. `syncFollowers` would do this on its next tick, but
    // a beat of two audio tracks is audible.
    previous.muted = true;
  }
  leaderSource = next;
  const v = leaderVideo();
  if (v) for (const [type, fn] of LEADER_EVENTS) v.addEventListener(type, fn);
  if (v && !v.paused) startTickers();
}

function startTickers() {
  saveTicker ??= setInterval(() => void saveLectureProgress(), SAVE_EVERY_MS);
  // Only worth a timer when something is actually following.
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

/**
 * Put every follower back where the leader is. Both files are the same
 * recording trimmed the same way (`echo360::TRIM_SECS`), so "in sync" is
 * simply the same `currentTime` — there is no offset to carry.
 */
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

/**
 * Put one follower back on the leader's clock: a seek only when it is a long
 * way out or the leader is not moving, and otherwise a trim to its rate that
 * closes the gap without a re-buffer.
 */
function alignFollower(f: HTMLVideoElement, lead: HTMLVideoElement) {
  const rate = lead.playbackRate;
  const behind = lead.currentTime - f.currentTime;
  const off = Math.abs(behind);

  // Paused, there is no rate to ride: land on the frame, exactly.
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
  // Back together — hand the leader's rate back before it overshoots.
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
 * Point the elements at `lecture` and lay them out to match `plan`, whose
 * **first entry is the leader** — the frame whose audio you hear.
 *
 * This is a reconcile, not a series of commands: the player re-states what it
 * wants on screen whenever anything changes (a different lecture, a layout, a
 * swapped source, a tab) and everything else follows. Which means the two
 * awkward moments are handled in one place:
 *
 *   * **a source joins** — it loads and `syncFollowers` walks it onto the
 *     leader's clock within a tick;
 *   * **the leader changes** — switching the single view from screen to camera
 *     is a different file with a different decoder, so the position and the
 *     playing/paused state are carried across by hand.
 *
 * Returns the leader element, which is what the player wires its own UI to.
 */
export function syncLectureSources(
  lecture: Lecture,
  plan: SourcePlan[],
  tabId: number,
): HTMLVideoElement | null {
  ownerTab = tabId;
  claim++;

  const lectureChanged = current?.id !== lecture.id;
  // A different lecture takes the elements over, so the old one's position
  // goes down before the sources change under it.
  if (lectureChanged && current) void saveLectureProgress();
  current = lecture;
  if (lectureChanged) lastWritten = -1;

  // Where playback is *now*, to hand to whatever takes over. Null when there
  // is nothing to carry — a fresh lecture starts from its saved progress.
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
      // Either carry the live position over, or restore the saved one. The
      // player used to do the latter on `loadedmetadata`, which fought this:
      // a source switch mid-lecture would land on the last *written* second
      // rather than the one you were watching.
      joinAt(v, at ?? lecture.progress_seconds, wasPlaying && p.source === plan[0].source);
    }
    if (v.parentElement !== p.host) p.host.appendChild(v);
    onScreen.add(p.source);
  }

  setLeader(plan[0]?.source ?? null);
  const lead = leaderVideo();
  // A leader that was *already* loaded and is taking over from another source
  // still has to be moved to where that one was. A fresh one was handed the
  // position above, on the load it is waiting for.
  if (lead && !leaderIsFresh && at != null && Math.abs(lead.currentTime - at) > MAX_DRIFT) {
    joinAt(lead, at, wasPlaying);
  }
  syncFollowers();
  if (lead && !lead.paused) startTickers();
  return lead;
}

/** The claim the last `syncLectureSources` handed out. A player keeps its own
 *  and gives it back when it parks, so a stale one parks nothing. */
export function playbackClaim(): number {
  return claim;
}

/** Is a lecture actually running right now (as opposed to loaded and paused)? */
export function isLecturePlaying(): boolean {
  const v = leaderVideo();
  return !!v && !v.paused && !v.ended;
}

/** The lecture playing now, for a dialog that has to name it. */
export function playingLecture(): Lecture | null {
  return isLecturePlaying() ? current : null;
}

/** Whether this tab is the one playback belongs to. */
export function ownsPlayback(tabId: number): boolean {
  return ownerTab === tabId;
}

/**
 * Hand every element back to the off-screen host — still playing, if it was.
 *
 * `forClaim` is a player parking its own: give it the claim the matching
 * `syncLectureSources` returned and the call is a no-op once someone else has
 * taken the elements over. Omitted, it parks whatever is out there, which is
 * what closing a lecture wants.
 */
export function parkLectureVideos(forClaim?: number) {
  if (forClaim != null && forClaim !== claim) return;
  if (!leaderVideo()) return;
  if (!isLecturePlaying()) void saveLectureProgress();
  for (const v of els.values()) parked().appendChild(v);
}

/**
 * Stop for good — the lecture was closed, not navigated away from. Leaving a
 * lecture's tab is a "keep it going"; closing it is not.
 */
export function stopLecturePlayback() {
  if (!els.size) return;
  for (const v of els.values()) v.pause();
  stopTickers();
  void saveLectureProgress();
  parkLectureVideos();
  ownerTab = null;
}

/** Write where the loaded lecture is now. Safe to call at any time. */
export async function saveLectureProgress(): Promise<void> {
  const v = leaderVideo();
  const lecture = current;
  if (!v || !lecture) return;
  const seconds = Math.floor(v.currentTime);
  if (seconds === lastWritten) return;
  lastWritten = seconds;
  try {
    await updateLectureProgress(lecture.id, seconds);
    // The file's length beats the catalogue's — Echo360's lesson duration runs
    // a few seconds short of the recording it serves.
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

// A hidden page is a throttled page: WebKit keeps the audio running but stops
// handing the pictures frames, so the two elements come back apart by however
// long you were away. That is far past `SEEK_DRIFT`, and one alignment on the
// way in is the cheap version of the tick discovering it a second later.
document.addEventListener("visibilitychange", () => {
  if (document.hidden) return;
  if (leaderVideo()) syncFollowers();
});

// Quitting mid-lecture should still land where you were. Best effort: the
// write is async and the webview is going away, but it usually beats it.
window.addEventListener("pagehide", () => void saveLectureProgress());
