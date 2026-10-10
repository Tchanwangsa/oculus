import { markLectureComplete, updateLectureProgress } from "@/lib/db";
import { isWatched } from "@/lib/lectures/end";
import { leaderVideo, state } from "./source";

/** Fired after a progress write so a mounted list can refresh its rows. */
export const LECTURE_PROGRESS_EVENT = "oculus-lecture-progress";

export function saveLectureProgress(): Promise<void> {
  const v = leaderVideo();
  const lecture = state.current;
  if (!v || !lecture) return state.saving;
  const seconds = Math.floor(v.currentTime);
  if (seconds === state.lastWritten) return state.saving;
  state.lastWritten = seconds;
  // The file's length beats Echo360's catalogue duration, which runs short.
  const fileDuration = Number.isFinite(v.duration) && v.duration > 0 ? v.duration : 0;
  const watched = isWatched(lecture, v.currentTime, fileDuration);
  state.saving = state.saving.then(() => writeProgress(lecture.id, seconds, watched));
  return state.saving;
}

async function writeProgress(id: string, seconds: number, watched: boolean): Promise<void> {
  try {
    await updateLectureProgress(id, seconds);
    if (watched) await markLectureComplete(id);
    window.dispatchEvent(new CustomEvent(LECTURE_PROGRESS_EVENT, { detail: { id, seconds } }));
  } catch {
    /* a lost position is not worth an error in the player */
  }
}

// Best effort: the write is async and the webview is going away.
window.addEventListener("pagehide", () => void saveLectureProgress());
