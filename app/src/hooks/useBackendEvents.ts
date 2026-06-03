import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  setParseStatus, upsertFile, markSubjectSynced, finishSyncRun, addLog,
} from "@/lib/db";
import { useSyncStore } from "@/stores/syncStore";
import { useParseStore } from "@/stores/parseStore";
import type { SyncProgress } from "@/stores/syncStore";
import type { ParseJob } from "@/stores/parseStore";

/**
 * Single app-level bridge: subscribes to all backend Tauri events and writes
 * them into the global stores (and the DB). Mount ONCE near the app root so
 * progress survives page navigation. UI reads from the stores, never listens
 * directly.
 */
export function useBackendEvents() {
  useEffect(() => {
    const unsubs: Array<Promise<() => void>> = [];

    // ── Sync (scrape) events ────────────────────────────────────────────────
    unsubs.push(
      listen<SyncProgress>("scrape-progress", (e) => {
        if (e.payload.phase === "complete") return;
        useSyncStore.getState().setProgress(e.payload);
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
      }),
    );
    unsubs.push(
      listen<string>("scrape-error", async (e) => {
        const runId = useSyncStore.getState().runId;
        if (runId != null) await finishSyncRun(runId, "failed", 0, 0, e.payload);
        useSyncStore.getState().fail(typeof e.payload === "string" ? e.payload : "Sync error");
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
      }),
    );

    return () => {
      unsubs.forEach((u) => u.then((f) => f()));
    };
  }, []);
}
