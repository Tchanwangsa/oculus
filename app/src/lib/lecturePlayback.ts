import {
  markLectureComplete,
  updateLectureProgress,
  type Lecture,
} from "@/lib/db";

/**
 * One `<video>` element for the whole app, owned here rather than by the
 * player component.
 *
 * A tab switch is a route change, and a route change unmounts the page —
 * which used to take the video element, and the lecture, with it. The element
 * living out here instead means switching tabs (or expanding the peek into its
 * own tab, which is the same unmount) hands the element from one host to the
 * next without interrupting playback: the lecture keeps going in the
 * background and picks up on screen exactly where it is when you come back.
 *
 * This is a DOM node in the *visible* webview, not a hidden WebView — the
 * suspension problem that keeps scraping in Rust does not apply to it.
 *
 * Progress is written from here too, for the same reason: the position has to
 * keep being saved while no player is mounted to save it.
 */

/** Fired after a progress write so a mounted list can refresh its rows. */
export const LECTURE_PROGRESS_EVENT = "oculus-lecture-progress";

/** How often a playing lecture writes down where it is. */
const SAVE_EVERY_MS = 5000;

/** Ending within this of the finish counts as watched. */
const COMPLETE_WITHIN = 30;

let video: HTMLVideoElement | null = null;
let parkedHost: HTMLDivElement | null = null;
let ticker: ReturnType<typeof setInterval> | null = null;
/** The lecture the element is loaded with, for the writes below. */
let current: Lecture | null = null;
/** Last second written, so a paused element doesn't rewrite the same row. */
let lastWritten = -1;
/**
 * The tab the player was mounted in when it took the element. Playback belongs
 * to that tab: switching to another one leaves it alone, while closing it or
 * navigating it elsewhere is leaving the lecture, and asks first.
 */
let ownerTab: number | null = null;

/**
 * Where the element sits while no player is mounted. Off screen rather than
 * `display: none`: a hidden subtree is where a browser feels entitled to stop
 * a media element, and this host exists precisely so it doesn't stop.
 */
function parked(): HTMLDivElement {
  if (!parkedHost) {
    const host = document.createElement("div");
    host.setAttribute("aria-hidden", "true");
    host.style.cssText =
      "position:fixed;left:-10000px;top:0;width:1px;height:1px;overflow:hidden;opacity:0;pointer-events:none";
    document.body.appendChild(host);
    parkedHost = host;
  }
  return parkedHost;
}

function startTicker() {
  if (ticker) return;
  ticker = setInterval(() => void saveLectureProgress(), SAVE_EVERY_MS);
}

function stopTicker() {
  if (!ticker) return;
  clearInterval(ticker);
  ticker = null;
}

/** The shared element, created on first use. */
export function lectureVideo(): HTMLVideoElement {
  if (video) return video;
  const v = document.createElement("video");
  v.playsInline = true;
  v.preload = "metadata";
  v.className = "w-full h-full object-contain";
  // Playing writes every few seconds; the moments that end a stretch of
  // playback write immediately, because they are exactly when the position
  // stops changing on its own.
  v.addEventListener("play", startTicker);
  v.addEventListener("pause", () => {
    stopTicker();
    void saveLectureProgress();
  });
  v.addEventListener("ended", () => {
    stopTicker();
    void saveLectureProgress();
  });
  v.addEventListener("seeked", () => void saveLectureProgress());
  parked().appendChild(v);
  video = v;
  return v;
}

/**
 * Point the shared element at a lecture. Returns whether it had to load the
 * file: `false` means this lecture is already loaded — you switched away and
 * came back — which is the caller's cue *not* to seek it back to the saved
 * position, since the live one is ahead of it.
 */
export function openLecture(lecture: Lecture, src: string): boolean {
  const v = lectureVideo();
  const same = v.dataset.lectureId === lecture.id && v.src === src;
  current = lecture;
  if (same) return false;
  // A different lecture takes the element over, so the old one's position goes
  // down before the source changes under it.
  const previous = v.dataset.lectureId;
  if (previous) void saveLectureProgress();
  current = lecture;
  lastWritten = -1;
  v.dataset.lectureId = lecture.id;
  v.src = src;
  return true;
}

/** Move the element into a mounted player's frame, in the tab that owns it. */
export function adoptLectureVideo(host: HTMLElement, tabId: number) {
  const v = lectureVideo();
  ownerTab = tabId;
  if (v.parentElement !== host) host.appendChild(v);
}

/** Is a lecture actually running right now (as opposed to loaded and paused)? */
export function isLecturePlaying(): boolean {
  return !!video && !video.paused && !video.ended;
}

/** The lecture playing now, for a dialog that has to name it. */
export function playingLecture(): Lecture | null {
  return isLecturePlaying() ? current : null;
}

/** Whether this tab is the one playback belongs to. */
export function ownsPlayback(tabId: number): boolean {
  return ownerTab === tabId;
}

/** Hand it back to the off-screen host — still playing, if it was. */
export function parkLectureVideo() {
  if (!video) return;
  if (video.paused) void saveLectureProgress();
  parked().appendChild(video);
}

/**
 * Stop for good — the lecture was closed, not navigated away from. Leaving a
 * lecture's tab is a "keep it going"; closing it is not.
 */
export function stopLecturePlayback() {
  if (!video) return;
  video.pause();
  void saveLectureProgress();
  parkLectureVideo();
  ownerTab = null;
}

/** Write where the loaded lecture is now. Safe to call at any time. */
export async function saveLectureProgress(): Promise<void> {
  const v = video;
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

// Quitting mid-lecture should still land where you were. Best effort: the
// write is async and the webview is going away, but it usually beats it.
window.addEventListener("pagehide", () => void saveLectureProgress());
