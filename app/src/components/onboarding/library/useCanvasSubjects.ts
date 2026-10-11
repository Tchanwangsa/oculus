import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTauriEvent } from "@/hooks/backend/useEvents";
import {
  addLog,
  getSubjects,
  setSubjectSelected,
  upsertSubjects,
  type CanvasCourseRaw,
  type Subject,
} from "@/lib/db";

/**
 * The Library step's subjects. Asks `get_auth_status` (not `useAuth`, whose
 * first answer is "disconnected"); when Canvas is connected, shows what the
 * database already holds and fetches the course list once. The fetch answers
 * by event, and the Sync page that usually upserts it isn't mounted during
 * setup, so this does. New subjects arrive selected only if current-term
 * (`upsertSubjects`); a re-run keeps earlier choices.
 */
export function useCanvasSubjects() {
  const [canvas, setCanvas] = useState<boolean | null>(null);
  const [subjects, setSubjects] = useState<Subject[]>([]);
  const [fetching, setFetching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // StrictMode mounts twice; one Canvas request is enough.
  const asked = useRef(false);

  const reload = useCallback(async () => {
    try {
      setSubjects(await getSubjects());
    } catch (cause) {
      console.error("subjects read failed", cause);
      setError(String(cause));
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    invoke<boolean>("get_auth_status")
      .catch(() => false)
      .then((connected) => {
        if (!cancelled) setCanvas(connected);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!canvas || asked.current) return;
    asked.current = true;
    void reload();
    setFetching(true);
    setError(null);
    invoke("sync_subjects").catch((cause) => {
      setFetching(false);
      setError(String(cause));
    });
  }, [canvas, reload]);

  useTauriEvent<CanvasCourseRaw[]>("subjects-loaded", async (e) => {
    try {
      await upsertSubjects(e.payload);
      await addLog(`Fetched ${e.payload.length} subjects from Canvas`);
      await reload();
      setError(null);
    } catch (cause) {
      console.error("subjects upsert failed", cause);
      setError(String(cause));
    } finally {
      setFetching(false);
    }
  });
  useTauriEvent<string>("subjects-error", (e) => {
    setFetching(false);
    setError(e.payload);
  });

  const toggle = (id: number) => {
    const subject = subjects.find((s) => s.id === id);
    if (!subject) return;
    const selected = !subject.selected;
    setSubjects((prev) => prev.map((s) => (s.id === id ? { ...s, selected } : s)));
    setSubjectSelected(id, selected).catch((cause) => {
      console.error("persist subject selection failed", cause);
      setSubjects((prev) => prev.map((s) => (s.id === id ? { ...s, selected: !selected } : s)));
    });
  };

  const selectedCount = subjects.filter((s) => s.selected).length;

  return { canvas, subjects, fetching, error, toggle, selectedCount };
}
