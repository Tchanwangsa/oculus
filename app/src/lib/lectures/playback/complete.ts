import { markLectureComplete } from "@/lib/db";
import { saveLectureProgress } from "./progress";
import { leaderVideo, state } from "./source";

/**
 * Mark the playing lecture Done for Up Next's Play. It is paused and its
 * position written first, so no progress write for it can land after.
 */
export async function completeLecture(id: string): Promise<void> {
  const v = leaderVideo();
  if (state.current?.id === id && v && !v.paused) v.pause();
  await saveLectureProgress();
  state.saving = state.saving.then(() => markLectureComplete(id)).catch(() => {});
  await state.saving;
}

/** The next adoption of `id` plays it, from `at` if given, else from its
 *  saved position. Up Next sets it before replacing the route. */
export function playOnAdopt(id: string, at?: number): void {
  state.autoplay = { id, at };
}
