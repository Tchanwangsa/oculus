import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { getRecap, getRecapStatus, type RecapNote } from "@/lib/db";
import {
  LECTURE_RECAP_EVENT,
  LECTURE_RECAP_PROGRESS_EVENT,
  writeLectureRecap,
  type RecapRunFinished,
  type RecapRunProgress,
} from "@/lib/lectures";

/** The `recap_status` column, with `NULL` given a name. */
export type RecapStatus = "none" | "running" | "ready" | "error";

/** When a run this session started was claimed, keyed by lecture — the same
 *  module-level map `useLectureChapters` keeps, for the same reason: the
 *  player unmounts on every tab switch and the job outlives it by many
 *  minutes. Nothing about it is persisted, so a run already in flight at app
 *  launch is shown without a clock rather than with a wrong one. */
const startedAt = new Map<string, number>();

/** The last step each in-flight run reported, with the lifetime of
 *  `startedAt`: a remounted panel would otherwise sit on "Writing a recap"
 *  with no detail until the agent happened to open its next file. */
const lastStep = new Map<string, RecapRunProgress>();

export interface RecapState {
  notes: RecapNote[];
  status: RecapStatus;
  /** `recap_error` — the message the failed run left behind. */
  error: string | null;
  since: number | null;
  progress: RecapRunProgress | null;
  busy: boolean;
  write: (force: boolean) => void;
}

/**
 * A lecture's recap notes and the job's state, read from SQLite and refreshed
 * on the backend's own events.
 *
 * **The one way this differs from `useLectureChapters`, and it is the
 * interesting one: a recap becomes visible while it is still being written.**
 * Chapters are validated all-or-nothing and written in a single transaction,
 * so there is nothing to show until the run ends. A recap commits per window
 * (docs/chapters.md), so the rows for the first ten minutes exist while the
 * agent is still reading the next ten — and a job that takes several turns is
 * exactly the one worth watching arrive. So the `writing` phase re-reads the
 * table rather than only painting a line.
 */
export function useLectureRecap(lectureId: string): RecapState {
  const [notes, setNotes] = useState<RecapNote[]>([]);
  const [status, setStatus] = useState<RecapStatus>("none");
  const [error, setError] = useState<string | null>(null);
  const [since, setSince] = useState<number | null>(null);
  const [progress, setProgress] = useState<RecapRunProgress | null>(null);
  const [starting, setStarting] = useState(false);

  const idRef = useRef(lectureId);
  idRef.current = lectureId;

  const reload = useCallback(async () => {
    const id = lectureId;
    const [rows, row] = await Promise.all([getRecap(id), getRecapStatus(id)]);
    if (idRef.current !== id) return;
    setNotes(rows);
    const s = row?.recap_status;
    setStatus(s === "running" || s === "ready" || s === "error" ? s : "none");
    setError(row?.recap_error ?? null);
    setSince(startedAt.get(id) ?? null);
    setProgress(s === "running" ? lastStep.get(id) ?? null : null);
  }, [lectureId]);

  useEffect(() => {
    setNotes([]);
    setStatus("none");
    setError(null);
    setSince(startedAt.get(lectureId) ?? null);
    setProgress(lastStep.get(lectureId) ?? null);
    reload();
  }, [lectureId, reload]);

  useEffect(() => {
    const unlisten = listen<RecapRunFinished>(LECTURE_RECAP_EVENT, (e) => {
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
    const unlisten = listen<RecapRunProgress>(LECTURE_RECAP_PROGRESS_EVENT, (e) => {
      lastStep.set(e.payload.lectureId, e.payload);
      if (e.payload.lectureId !== idRef.current) return;
      setProgress(e.payload);
      // A step is proof of a run: a `reload` that raced the claim and read the
      // old NULL would leave the panel offering a button for a job that is
      // already minutes into itself.
      setStatus("running");
      // A window has just been committed — see this hook's own comment. The
      // read is one indexed `SELECT` against a table with tens of rows in it,
      // and it fires once per window, not once per step.
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
    // Not cleared optimistically: `claim_recap` deletes the old set, and a
    // failure before that leaves it in place — so the notes on screen stay the
    // notes in the database either way.
    writeLectureRecap(id, force)
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
    notes,
    status,
    error,
    since,
    progress,
    busy: starting || status === "running",
    write,
  };
}
