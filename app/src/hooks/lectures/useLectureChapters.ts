import { getChapterStatus, getChapters, type Chapter } from "@/lib/db";
import {
  LECTURE_CHAPTERS_EVENT,
  LECTURE_CHAPTER_PROGRESS_EVENT,
  findLectureChapters,
  type ChapterRunProgress,
} from "@/lib/lectures";
import { lectureJob, useLectureJob, type LectureJobState, type LectureJobStatus } from "./useLectureJob";

export type ChapterStatus = LectureJobStatus;

type ChapterState = Omit<LectureJobState<Chapter, ChapterRunProgress>, "rows" | "run"> & {
  chapters: Chapter[];
  find: (force: boolean) => void;
};

/** Chapters are written in one transaction, so nothing shows until the run
 *  ends — see docs/chapters.md. */
const chaptersJob = lectureJob<Chapter, ChapterRunProgress>({
  load: async (id) => {
    const [rows, row] = await Promise.all([getChapters(id), getChapterStatus(id)]);
    return { rows, status: row?.chapter_status, error: row?.chapter_error };
  },
  doneEvent: LECTURE_CHAPTERS_EVENT,
  progressEvent: LECTURE_CHAPTER_PROGRESS_EVENT,
  start: findLectureChapters,
});

/** A lecture's chapters and the chaptering job's state. */
export function useLectureChapters(lectureId: string): ChapterState {
  const { rows, run, ...rest } = useLectureJob(chaptersJob, lectureId);
  return { ...rest, chapters: rows, find: run };
}
