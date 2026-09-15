import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { getChapterStatus, getChapters, type Chapter } from "@/lib/db";
import {
  LECTURE_CHAPTERS_EVENT,
  LECTURE_CHAPTER_PROGRESS_EVENT,
  findLectureChapters,
  type ChapterRunFinished,
  type ChapterRunProgress,
} from "@/lib/lectures";

/** The `chapter_status` column, with `NULL` given a name. */
export type ChapterStatus = "none" | "running" | "ready" | "error";

/**
 * When a run this session started was claimed, keyed by lecture.
 *
 * Module-level rather than component state because the player unmounts every
 * time you switch tabs and the job outlives it by eight minutes — an elapsed
 * clock that restarted at zero on every visit would be a worse lie than no
 * clock at all. There is no column for it: `chaptered_at` is stamped only by a
 * terminal status, so a run already in flight when the app started has no start
 * time anywhere and is shown without one.
 */
const startedAt = new Map<string, number>();

/**
 * The last step each in-flight run reported, for the same reason and with the
 * same lifetime as `startedAt`: the panel unmounts on every tab switch, and a
 * remounted one would sit on "Finding chapters" with no step until the agent
 * happened to open its next file — up to a minute of looking stalled.
 *
 * Nothing here is persisted. A run that was already in flight when the app
 * started has no entry and shows the phase-less spinner until its next step
 * arrives, which is the same trade `since` makes.
 */
const lastStep = new Map<string, ChapterRunProgress>();

export interface ChapterState {
  chapters: Chapter[];
  status: ChapterStatus;
  /** `chapter_error` — the message the failed run left behind. */
  error: string | null;
  /** Epoch ms this session claimed the run, or null (see `startedAt`). */
  since: number | null;
  /** What the run is doing right now, or null when nothing has been heard
   *  from it yet — a job started before this app launch, or the first second
   *  of one. */
  progress: ChapterRunProgress | null;
  /** The trigger is in flight, or the job is: either way, do not ask again. */
  busy: boolean;
  /** Regenerate is `force`; a fresh lecture is not. */
  find: (force: boolean) => void;
}

/**
 * A lecture's chapters and the job's state, read straight from SQLite and
 * refreshed on the backend's own `lecture-chapters` event.
 *
 * Two events, because they mean different things: `lecture-chapters` is a
 * result and costs a re-read of both tables, `lecture-chapter-progress` is a
 * step and costs a `setState` — see docs/chapters.md.
 *
 * The status deliberately does *not* come from the `lectures` row the player
 * was handed: in the side panel that row is a snapshot in a store, and in a
 * list it is whatever the last `getLectures` returned — neither is re-read when
 * a run lands eight minutes later.
 */
export function useLectureChapters(lectureId: string): ChapterState {
  const [chapters, setChapters] = useState<Chapter[]>([]);
  const [status, setStatus] = useState<ChapterStatus>("none");
  const [error, setError] = useState<string | null>(null);
  const [since, setSince] = useState<number | null>(null);
  const [progress, setProgress] = useState<ChapterRunProgress | null>(null);
  /** The invoke itself, which is short: Rust claims the run and returns. */
  const [starting, setStarting] = useState(false);

  const idRef = useRef(lectureId);
  idRef.current = lectureId;

  const reload = useCallback(async () => {
    const id = lectureId;
    const [rows, row] = await Promise.all([
      getChapters(id),
      getChapterStatus(id),
    ]);
    if (idRef.current !== id) return;
    setChapters(rows);
    const s = row?.chapter_status;
    setStatus(s === "running" || s === "ready" || s === "error" ? s : "none");
    setError(row?.chapter_error ?? null);
    setSince(startedAt.get(id) ?? null);
    setProgress(s === "running" ? lastStep.get(id) ?? null : null);
  }, [lectureId]);

  useEffect(() => {
    setChapters([]);
    setStatus("none");
    setError(null);
    setSince(startedAt.get(lectureId) ?? null);
    setProgress(lastStep.get(lectureId) ?? null);
    reload();
  }, [lectureId, reload]);

  // Rust's event, not `lectures-changed`: that one fires on every progress
  // save, and a result this long in the making would be lost in it.
  useEffect(() => {
    const unlisten = listen<ChapterRunFinished>(LECTURE_CHAPTERS_EVENT, (e) => {
      // The map is cleaned up for whichever lecture ended, even one this
      // player is not showing: the entry would otherwise outlive the run and
      // greet the next visit with a step from a job that finished hours ago.
      startedAt.delete(e.payload.lectureId);
      lastStep.delete(e.payload.lectureId);
      if (e.payload.lectureId !== idRef.current) return;
      reload();
    });
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, [reload]);

  // Every step of the run, from the ffmpeg decode through the agent's own tool
  // calls. Display only — nothing here is read back from the database, so a
  // missed event costs one frame of a line that is about to be replaced.
  useEffect(() => {
    const unlisten = listen<ChapterRunProgress>(
      LECTURE_CHAPTER_PROGRESS_EVENT,
      (e) => {
        lastStep.set(e.payload.lectureId, e.payload);
        if (e.payload.lectureId !== idRef.current) return;
        setProgress(e.payload);
        // A step is proof of a run: a `reload` that raced the claim and read
        // the old NULL would otherwise leave the panel offering a button for
        // a job that is already several minutes into itself.
        setStatus("running");
      },
    );
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, []);

  const find = useCallback(
    (force: boolean) => {
      const id = idRef.current;
      setStarting(true);
      // Optimistic: Rust claims the column on its own thread, so re-reading
      // straight away can still see the old value. The event corrects both.
      startedAt.set(id, Date.now());
      lastStep.delete(id);
      setSince(startedAt.get(id) ?? null);
      setProgress(null);
      setStatus("running");
      setError(null);
      findLectureChapters(id, force)
        .catch((e) => {
          startedAt.delete(id);
          lastStep.delete(id);
          if (idRef.current !== id) return;
          setStatus("error");
          setError(String(e));
        })
        .finally(() => setStarting(false));
    },
    [],
  );

  return {
    chapters,
    status,
    error,
    since,
    progress,
    busy: starting || status === "running",
    find,
  };
}
