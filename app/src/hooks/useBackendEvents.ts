import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  setParseStatus, upsertFile, markSubjectSynced, finishSyncRun, addLog,
} from "@/lib/db";
import { useSyncStore } from "@/stores/syncStore";
import { useParseStore } from "@/stores/parseStore";
import { useJobStore } from "@/stores/jobStore";
import type { SyncProgress } from "@/stores/syncStore";
import type { ParseJob } from "@/stores/parseStore";

/**
 * Single app-level bridge: subscribes to all backend Tauri events and writes
 * them into the global stores (and the DB). Mount ONCE near the app root so
 * progress survives page navigation. UI reads from the stores, never listens
 * directly.
 */
export function useBackendEvents() {
  // PDF parse aggregate tracking — survives re-renders, reset on mount
  const pdfSeen = useRef(new Set<string>());  // all paths ever queued
  const pdfCompleted = useRef(new Set<string>());  // paths that finished (fast/quality/error)

  useEffect(() => {
    const unsubs: Array<Promise<() => void>> = [];

    // ── Sync (scrape) events ────────────────────────────────────────────────
    unsubs.push(
      listen<SyncProgress>("scrape-progress", (e) => {
        if (e.payload.phase === "complete") return;
        const p = e.payload;
        useSyncStore.getState().setProgress(p);
        useJobStore.getState().upsert({
          id: "canvas_sync",
          type: "canvas_sync",
          status: "running",
          label: p.course,
          progress_current: p.done,
          progress_total: p.total,
        });
      }),
    );
    unsubs.push(
      listen<{ subject_id: number; relative_path: string; size_bytes: number; category: string | null; canvas_id: number | null }>(
        "scrape-file",
        async (e) => {
          const { subject_id, relative_path, size_bytes, category, canvas_id } = e.payload;
          const filename = relative_path.split("/").pop() ?? relative_path;
          const ext = filename.includes(".") ? filename.split(".").pop()! : "md";
          try {
            await upsertFile(subject_id, filename, relative_path, ext, size_bytes, category ?? undefined, canvas_id ?? undefined);
            await markSubjectSynced(subject_id);
          } catch { /* ignore */ }
        },
      ),
    );
    unsubs.push(
      listen<{ level: string; course: string; message: string }>("scrape-log", (e) => {
        const { level, message } = e.payload;
        const mapped = level === "error" ? "error" : level === "warning" ? "warning" : "info";
        addLog(message, mapped).catch(() => {});
      }),
    );
    unsubs.push(
      listen<{ count: number; cancelled?: boolean }>("scrape-complete", async (e) => {
        const runId = useSyncStore.getState().runId;
        if (runId != null) {
          await finishSyncRun(runId, "completed", e.payload.count, e.payload.count);
          await addLog(`Synced ${e.payload.count} subject(s)`);
        }
        useSyncStore.getState().complete(e.payload.count, !!e.payload.cancelled);
        useJobStore.getState().remove("canvas_sync");
      }),
    );
    unsubs.push(
      listen<string>("scrape-error", async (e) => {
        const runId = useSyncStore.getState().runId;
        if (runId != null) await finishSyncRun(runId, "failed", 0, 0, e.payload);
        useSyncStore.getState().fail(typeof e.payload === "string" ? e.payload : "Sync error");
        useJobStore.getState().remove("canvas_sync");
      }),
    );

    // ── PDF parse events ────────────────────────────────────────────────────
    unsubs.push(
      listen<ParseJob>("parse-status", async (e) => {
        const ev = e.payload;
        useParseStore.getState().update(ev);
        try {
          await setParseStatus(ev.subject_id, ev.relative_path, ev.status);
        } catch { /* ignore */ }

        // Aggregate into jobStore
        const path = ev.relative_path;
        const filename = path.split("/").pop() ?? path;
        const isTerminal = ev.status === "fast" || ev.status === "quality" || ev.status === "error";

        pdfSeen.current.add(path);
        if (isTerminal) pdfCompleted.current.add(path);

        const total = pdfSeen.current.size;
        const done = pdfCompleted.current.size;

        if (done >= total && total > 0) {
          useJobStore.getState().upsert({
            id: "pdf_parse",
            type: "pdf_parse",
            status: "completed",
            progress_current: done,
            progress_total: total,
          });
          setTimeout(() => useJobStore.getState().remove("pdf_parse"), 1500);
        } else {
          useJobStore.getState().upsert({
            id: "pdf_parse",
            type: "pdf_parse",
            status: "running",
            label: filename,
            progress_current: done,
            progress_total: total,
          });
        }
      }),
    );

    return () => {
      unsubs.forEach((u) => u.then((f) => f()));
    };
  }, []);
}
