import { getReading, getReadingStatus, type ReadingLine } from "@/lib/db";
import {
  LECTURE_READING_EVENT,
  LECTURE_READING_PROGRESS_EVENT,
  writeLectureReading,
  type ReadingRunProgress,
} from "@/lib/lectures";
import { lectureJob, useLectureJob, type LectureJobState, type LectureJobStatus } from "./useLectureJob";

export type ReadingStatus = LectureJobStatus;

type ReadingState = Omit<LectureJobState<ReadingLine, ReadingRunProgress>, "rows" | "run"> & {
  lines: ReadingLine[];
  write: (force: boolean) => void;
};

/** Unlike chapters, a reading copy commits per window (docs/chapters.md), so
 *  each `writing` step re-reads the table and lines appear while it runs. */
const readingJob = lectureJob<ReadingLine, ReadingRunProgress>({
  load: async (id) => {
    const [rows, row] = await Promise.all([getReading(id), getReadingStatus(id)]);
    return { rows, status: row?.reading_status, error: row?.reading_error };
  },
  doneEvent: LECTURE_READING_EVENT,
  progressEvent: LECTURE_READING_PROGRESS_EVENT,
  start: writeLectureReading,
  reloadOnStep: (p) => p.phase === "writing",
});

/** A lecture's reading copy and the reading-copy job's state. */
export function useLectureReading(lectureId: string): ReadingState {
  const { rows, run, ...rest } = useLectureJob(readingJob, lectureId);
  return { ...rest, lines: rows, write: run };
}
