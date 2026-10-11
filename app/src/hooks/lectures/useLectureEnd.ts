import { useCallback, useEffect, useSyncExternalStore } from "react";

import {
  lectureEndState,
  openLectureEnd,
  retryLectureEnd,
  subscribeLectureEnds,
  type LectureEndState,
} from "@/lib/lectures/end";

/**
 * The player's side of the end job: on open, read the end from SQLite and run
 * the job the first time (`openLectureEnd`); re-check when a transcript
 * arrives. Re-renders when an end lands, so Up Next moves with it.
 */
export function useLectureEnd(
  lectureId: string,
  transcriptPath: string | null,
): { state: LectureEndState | undefined; retry: () => void } {
  const state = useSyncExternalStore(subscribeLectureEnds, () => lectureEndState(lectureId));

  useEffect(() => {
    void openLectureEnd(lectureId);
  }, [lectureId, transcriptPath]);

  const retry = useCallback(() => retryLectureEnd(lectureId), [lectureId]);
  return { state, retry };
}
