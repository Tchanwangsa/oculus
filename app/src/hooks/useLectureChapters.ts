import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { getChapterStatus, getChapters, type Chapter } from "@/lib/db";
import {
  LECTURE_CHAPTERS_EVENT,
  findLectureChapters,
  type ChapterRunFinished,
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

export interface ChapterState {
  chapters: Chapter[];
  status: ChapterStatus;
  /** `chapter_error` — the message the failed run left behind. */
  error: string | null;
  /** Epoch ms this session claimed the run, or null (see `startedAt`). */
  since: number | null;
  /** The trigger is in flight, or the job is: either way, do not ask again. */
  busy: boolean;
  /** Regenerate is `force`; a fresh lecture is not. */
  find: (force: boolean) => void;
}

/**
 * A lecture's chapters and the job's state, read straight from SQLite and
 * refreshed on the backend's own `lecture-chapters` event.
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
  }, [lectureId]);

  useEffect(() => {
    setChapters([]);
    setStatus("none");
    setError(null);
    setSince(startedAt.get(lectureId) ?? null);
    reload();
  }, [lectureId, reload]);

  // Rust's event, not `lectures-changed`: that one fires on every progress
  // save, and a result this long in the making would be lost in it.
  useEffect(() => {
    const unlisten = listen<ChapterRunFinished>(LECTURE_CHAPTERS_EVENT, (e) => {
      if (e.payload.lectureId !== idRef.current) return;
      startedAt.delete(e.payload.lectureId);
      reload();
    });
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, [reload]);

  const find = useCallback(
    (force: boolean) => {
      const id = idRef.current;
      setStarting(true);
      // Optimistic: Rust claims the column on its own thread, so re-reading
      // straight away can still see the old value. The event corrects both.
      startedAt.set(id, Date.now());
      setSince(startedAt.get(id) ?? null);
      setStatus("running");
      setError(null);
      findLectureChapters(id, force)
        .catch((e) => {
          startedAt.delete(id);
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
    busy: starting || status === "running",
    find,
  };
}
