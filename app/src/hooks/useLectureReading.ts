import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { getReading, getReadingStatus, type ReadingLine } from "@/lib/db";
import {
  LECTURE_READING_EVENT,
  LECTURE_READING_PROGRESS_EVENT,
  writeLectureReading,
  type ReadingRunFinished,
  type ReadingRunProgress,
} from "@/lib/lectures";

/** The `reading_status` column, with `NULL` given a name. */
export type ReadingStatus = "none" | "running" | "ready" | "error";

/** When a run this session started was claimed, keyed by lecture — the same
 *  module-level map `useLectureChapters` keeps, for the same reason: the
 *  player unmounts on every tab switch and the job outlives it by many
 *  minutes. Nothing about it is persisted, so a run already in flight at app
 *  launch is shown without a clock rather than with a wrong one. */
const startedAt = new Map<string, number>();

/** The last step each in-flight run reported, with the lifetime of
 *  `startedAt`: a remounted panel would otherwise sit on "Writing the reading
 *  copy" with no detail until the agent happened to open its next file. */
const lastStep = new Map<string, ReadingRunProgress>();

export interface ReadingState {
  lines: ReadingLine[];
  status: ReadingStatus;
  /** `reading_error` — the message the failed run left behind. */
  error: string | null;
  since: number | null;
  progress: ReadingRunProgress | null;
  busy: boolean;
  write: (force: boolean) => void;
}

/**
 * A lecture's reading copy and the job's state, read from SQLite and
 * refreshed on the backend's own events.
 *
 * **The one way this differs from `useLectureChapters`, and it is the
 * interesting one: a reading copy becomes visible while it is still being
 * written.** Chapters are validated all-or-nothing and written in a single
 * transaction, so there is nothing to show until the run ends. A reading copy
 * commits per window (docs/chapters.md), so the lines for the first ten
 * minutes exist while the agent is still reading the next ten — and a job
 * that takes several turns is exactly the one worth watching arrive. So the
 * `writing` phase re-reads the table rather than only painting a line.
 */
export function useLectureReading(lectureId: string): ReadingState {
  const [lines, setLines] = useState<ReadingLine[]>([]);
  const [status, setStatus] = useState<ReadingStatus>("none");
  const [error, setError] = useState<string | null>(null);
  const [since, setSince] = useState<number | null>(null);
  const [progress, setProgress] = useState<ReadingRunProgress | null>(null);
  const [starting, setStarting] = useState(false);

  const idRef = useRef(lectureId);
  idRef.current = lectureId;

  const reload = useCallback(async () => {
    const id = lectureId;
    const [rows, row] = await Promise.all([getReading(id), getReadingStatus(id)]);
    if (idRef.current !== id) return;
    setLines(rows);
    const s = row?.reading_status;
    setStatus(s === "running" || s === "ready" || s === "error" ? s : "none");
    setError(row?.reading_error ?? null);
    setSince(startedAt.get(id) ?? null);
    setProgress(s === "running" ? lastStep.get(id) ?? null : null);
  }, [lectureId]);

  useEffect(() => {
    setLines([]);
    setStatus("none");
    setError(null);
    setSince(startedAt.get(lectureId) ?? null);
    setProgress(lastStep.get(lectureId) ?? null);
    reload();
  }, [lectureId, reload]);

  useEffect(() => {
    const unlisten = listen<ReadingRunFinished>(LECTURE_READING_EVENT, (e) => {
      // Cleaned up for whichever lecture ended, even one this player is not
      // showing: the entry would otherwise greet the next visit with a step
      // from a job that finished hours ago.
      startedAt.delete(e.payload.lectureId);
      lastStep.delete(e.payload.lectureId);
      if (e.payload.lectureId !== idRef.current) return;
      reload();
    });
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, [reload]);

  useEffect(() => {
    const unlisten = listen<ReadingRunProgress>(LECTURE_READING_PROGRESS_EVENT, (e) => {
      lastStep.set(e.payload.lectureId, e.payload);
      if (e.payload.lectureId !== idRef.current) return;
      setProgress(e.payload);
      // A step is proof of a run: a `reload` that raced the claim and read the
      // old NULL would leave the panel offering a button for a job that is
      // already minutes into itself.
      setStatus("running");
      // A window has just been committed — see this hook's own comment. The
      // read is one indexed `SELECT` against a table with hundreds of rows in
      // it, and it fires once per window, not once per step.
      if (e.payload.phase === "writing") reload();
    });
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, [reload]);

  const write = useCallback((force: boolean) => {
    const id = idRef.current;
    setStarting(true);
    // Optimistic: Rust claims the column on its own thread, so re-reading
    // straight away can still see the old value. The events correct both.
    startedAt.set(id, Date.now());
    lastStep.delete(id);
    setSince(startedAt.get(id) ?? null);
    setProgress(null);
    setStatus("running");
    setError(null);
    // Not cleared optimistically: `claim_reading` deletes the old set, and a
    // failure before that leaves it in place — so the lines on screen stay the
    // lines in the database either way.
    writeLectureReading(id, force)
      .catch((e) => {
        startedAt.delete(id);
        lastStep.delete(id);
        if (idRef.current !== id) return;
        setStatus("error");
        setError(String(e));
      })
      .finally(() => setStarting(false));
  }, []);

  return {
    lines,
    status,
    error,
    since,
    progress,
    busy: starting || status === "running",
    write,
  };
}
