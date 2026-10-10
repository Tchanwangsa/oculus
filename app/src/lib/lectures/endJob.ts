import { invoke } from "@tauri-apps/api/core";

/** Tauri event from `lecture_end::app::LECTURE_END_EVENT`: a run finished. */
export const LECTURE_END_EVENT = "lecture-end";

/** `seconds` and `quote` are set only when `status` is `ready`. */
export interface LectureEndFinished {
  lectureId: string;
  status: "ready" | "none" | "error";
  seconds: number | null;
  quote: string | null;
  error: string | null;
}

/**
 * Find where a lecture's content ends (the `lectureEnd` job, as `oculus
 * lecture end`). Needs the transcript only. Returns once claimed; the end
 * arrives as `LECTURE_END_EVENT`.
 */
export function findLectureEnd(lectureId: string, force = false): Promise<void> {
  return invoke("lecture_find_end", { lectureId, force });
}
