/**
 * The app's `<video>` elements and progress writes, owned here rather than by
 * the player so a route change (tab switch, the side panel's expand) hands
 * them to the next player without interrupting playback.
 *
 * One element per Echo360 source (`docs/sync.md`). Exactly one is the
 * **leader** — audio, clock, progress writes; the others follow it, muted.
 * Which player may adopt them is `lib/lectures/playbackOwner.ts`'s call.
 */

export { LECTURE_PROGRESS_EVENT } from "./progress";
export { videoForSource, type SourcePlan } from "./source";
export { syncLectureSources } from "./sync";
export {
  isLecturePlaying,
  ownsPlayback,
  parkLectureVideos,
  pauseLecturePlayback,
  playbackClaim,
  playingLecture,
  releaseLecturePlayer,
  stopLecturePlayback,
} from "./ownership";
export { completeLecture, playOnAdopt } from "./complete";
