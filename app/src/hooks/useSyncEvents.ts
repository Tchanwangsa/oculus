import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  upsertFile,
  markSubjectSynced,
  addLog,
  type CanvasCourseRaw,
} from "@/lib/db";

interface ScrapeFilePayload {
  subject_id: number;
  relative_path: string;
  size_bytes: number;
  category: string | null;
  canvas_id: number | null;
}

interface ScrapeProgressPayload {
  done: number;
  total: number;
  course?: string;
  phase?: string;
}

interface ScrapeLogPayload {
  level: string;
  course: string;
  message: string;
}

interface ScrapeCompletePayload {
  count: number;
  cancelled?: boolean;
}

export function useSyncEvents(handlers: {
  onScrapeFile?: (payload: ScrapeFilePayload) => void;
  onScrapeProgress?: (payload: ScrapeProgressPayload) => void;
  onScrapeLog?: (payload: ScrapeLogPayload) => void;
  onScrapeComplete?: (payload: ScrapeCompletePayload) => void;
  onScrapeError?: (err: string) => void;
  onSubjectsLoaded?: (courses: CanvasCourseRaw[]) => void;
  onSubjectsError?: (err: string) => void;
}) {
  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;

  useEffect(() => {
    const subs = [
      listen<CanvasCourseRaw[]>("subjects-loaded", (e) => {
        const raw = e.payload as unknown as CanvasCourseRaw[];
        handlersRef.current.onSubjectsLoaded?.(raw);
      }),
      listen<string>("subjects-error", (e) => {
        handlersRef.current.onSubjectsError?.(e.payload);
      }),
      listen<ScrapeFilePayload>("scrape-file", (e) => {
        handlersRef.current.onScrapeFile?.(e.payload);
      }),
      listen<ScrapeProgressPayload>("scrape-progress", (e) => {
        handlersRef.current.onScrapeProgress?.(e.payload);
      }),
      listen<ScrapeLogPayload>("scrape-log", (e) => {
        handlersRef.current.onScrapeLog?.(e.payload);
      }),
      listen<ScrapeCompletePayload>("scrape-complete", (e) => {
        handlersRef.current.onScrapeComplete?.(e.payload);
      }),
      listen<string>("scrape-error", (e) => {
        handlersRef.current.onScrapeError?.(e.payload);
      }),
    ];
    return () => {
      subs.forEach((p) => p.then((f) => f()));
    };
  }, []);
}

export async function handleScrapeFileEvent(payload: ScrapeFilePayload) {
  const { subject_id, relative_path, size_bytes, category, canvas_id } = payload;
  const filename = relative_path.split("/").pop() ?? relative_path;
  const ext = filename.includes(".") ? filename.split(".").pop()! : "md";
  try {
    await upsertFile(
      subject_id,
      filename,
      relative_path,
      ext,
      size_bytes,
      category ?? undefined,
      canvas_id ?? undefined,
    );
    await markSubjectSynced(subject_id);
  } catch {
    /* ignore */
  }
}

export async function handleScrapeLogEvent(payload: ScrapeLogPayload) {
  const { level, message } = payload;
  const mapped =
    level === "error" ? "error" : level === "warning" ? "warning" : "info";
  await addLog(message, mapped).catch(() => {});
}
