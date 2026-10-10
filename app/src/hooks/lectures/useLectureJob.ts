import { useCallback, useEffect, useRef, useState } from "react";
import { useTauriEvent } from "@/hooks/backend/useEvents";

/** A job's `*_status` column, with `NULL` given a name. */
export type LectureJobStatus = "none" | "running" | "ready" | "error";

/**
 * One background lecture job, such as chaptering: how to read its rows and
 * status column, the Rust events that report it, and how to start it.
 * Status is read from SQLite rather than the `lectures` row the player was
 * handed — that row is a snapshot that is not re-read when a run lands.
 */
export interface LectureJob<Row, P extends { lectureId: string }> {
  load: (id: string) => Promise<{
    rows: Row[];
    status: string | null | undefined;
    error: string | null | undefined;
  }>;
  /** Result event: costs a re-read of rows and status. */
  doneEvent: string;
  /** Step event: display only, costs a `setState`. */
  progressEvent: string;
  start: (id: string, force: boolean) => Promise<unknown>;
  /**
   * When this session claimed each run, and each run's last step, keyed by
   * lecture. Module-level because the player unmounts on every tab switch and
   * the job outlives it; nothing is persisted, so a run already in flight at
   * launch shows no clock and no step until its next one.
   */
  startedAt: Map<string, number>;
  lastStep: Map<string, P>;
}

export function lectureJob<Row, P extends { lectureId: string }>(
  spec: Omit<LectureJob<Row, P>, "startedAt" | "lastStep">,
): LectureJob<Row, P> {
  return { ...spec, startedAt: new Map(), lastStep: new Map() };
}

export interface LectureJobState<Row, P> {
  rows: Row[];
  status: LectureJobStatus;
  /** The message the failed run left behind. */
  error: string | null;
  /** Epoch ms this session claimed the run, or null (see `startedAt`). */
  since: number | null;
  /** The run's latest step, or null when nothing has been heard yet. */
  progress: P | null;
  /** The trigger is in flight, or the job is: either way, do not ask again. */
  busy: boolean;
  /** `force` regenerates; a fresh lecture needs none. */
  run: (force: boolean) => void;
}

export function useLectureJob<Row, P extends { lectureId: string }>(
  job: LectureJob<Row, P>,
  lectureId: string,
): LectureJobState<Row, P> {
  const { startedAt, lastStep } = job;
  const [rows, setRows] = useState<Row[]>([]);
  const [status, setStatus] = useState<LectureJobStatus>("none");
  const [error, setError] = useState<string | null>(null);
  const [since, setSince] = useState<number | null>(null);
  const [progress, setProgress] = useState<P | null>(null);
  /** The invoke itself, which is short: Rust claims the run and returns. */
  const [starting, setStarting] = useState(false);

  const idRef = useRef(lectureId);
  idRef.current = lectureId;

  const reload = useCallback(async () => {
    const id = lectureId;
    const r = await job.load(id);
    if (idRef.current !== id) return;
    setRows(r.rows);
    const s = r.status;
    setStatus(s === "running" || s === "ready" || s === "error" ? s : "none");
    setError(r.error ?? null);
    setSince(startedAt.get(id) ?? null);
    setProgress(s === "running" ? lastStep.get(id) ?? null : null);
  }, [job, lectureId, startedAt, lastStep]);

  useEffect(() => {
    setRows([]);
    setStatus("none");
    setError(null);
    setSince(startedAt.get(lectureId) ?? null);
    setProgress(lastStep.get(lectureId) ?? null);
    reload();
  }, [lectureId, reload, startedAt, lastStep]);

  useTauriEvent<{ lectureId: string }>(job.doneEvent, (e) => {
    // Cleared for whichever lecture ended, shown or not, so a later visit
    // isn't greeted by a step from a finished job.
    startedAt.delete(e.payload.lectureId);
    lastStep.delete(e.payload.lectureId);
    if (e.payload.lectureId !== idRef.current) return;
    reload();
  });

  useTauriEvent<P>(job.progressEvent, (e) => {
    lastStep.set(e.payload.lectureId, e.payload);
    if (e.payload.lectureId !== idRef.current) return;
    setProgress(e.payload);
    // A step proves a run: a `reload` that raced the claim may have read
    // the old NULL and would otherwise offer the start button again.
    setStatus("running");
  });

  const run = useCallback(
    (force: boolean) => {
      const id = idRef.current;
      setStarting(true);
      // Optimistic: Rust claims the column on its own thread, so an immediate
      // re-read can still see the old value. The events correct both. Rows
      // are left on screen — they stay whatever the database holds.
      startedAt.set(id, Date.now());
      lastStep.delete(id);
      setSince(startedAt.get(id) ?? null);
      setProgress(null);
      setStatus("running");
      setError(null);
      job
        .start(id, force)
        .catch((e) => {
          startedAt.delete(id);
          lastStep.delete(id);
          if (idRef.current !== id) return;
          setStatus("error");
          setError(String(e));
        })
        .finally(() => setStarting(false));
    },
    [job, startedAt, lastStep],
  );

  return {
    rows,
    status,
    error,
    since,
    progress,
    busy: starting || status === "running",
    run,
  };
}
