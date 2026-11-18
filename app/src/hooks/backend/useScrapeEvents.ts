import { invoke } from "@tauri-apps/api/core";
import {
  upsertFile, finishSyncRun, addLog, deleteFileRow, getFileByRelativePath,
  addSyncRunFile, getSyncOptions, markFileContentChanged, resetFilePipeline,
  upsertLectures,
  type LectureData, type SyncFileAction,
} from "@/lib/db";
import { useSyncStore } from "@/stores/sync/syncStore";
import { NO_UPLOAD } from "@/stores/sync/pipelineStore";
import type { SyncProgress } from "@/stores/sync/syncStore";
import { isPdfBacked, isPipelineFile } from "@/lib/files/fileTypes";
import { CALENDAR_UPDATED_EVENT, syncCalendar } from "@/lib/planning/calendar";
import { FILE_SCRAPED_EVENT, type FileScraped } from "@/lib/pipeline/syncRunner";
import { forgetPageSnapshots } from "@/lib/whiteboard/snapshots";
import { useTauriEvent } from "@/hooks/backend/useEvents";
import { NO_ERROR, parseStatuses, pipeline } from "./pipelineEvents";

/** Sync (scrape) events: progress, per-file results, the run's end. */
export function useScrapeEvents() {
  useTauriEvent<{ runId: number; subjects: { id: number; code: string }[] }>("scrape-started", (e) => {
    useSyncStore.getState().begin(e.payload.runId, e.payload.subjects);
  });
  useTauriEvent<SyncProgress & { runId?: number }>("scrape-progress", (e) => {
    // A tagged progress event belongs only to the run this store tracks.
    if (e.payload.runId != null && e.payload.runId !== useSyncStore.getState().runId) return;
    if (e.payload.phase === "complete") return;
    useSyncStore.getState().setProgress(e.payload);
  });
  useTauriEvent<{ subject_id: number; relative_path: string; filename: string }>(
    "scrape-file-start",
    (e) => {
      const { subject_id, relative_path } = e.payload;
      if (!isPipelineFile(relative_path)) return;
      pipeline().touch(relative_path, subject_id, { download: "active" });
    },
  );
  // A started download that ended without bytes; the next sync fetches it again.
  useTauriEvent<{ subject_id: number; relative_path: string; error: string }>(
    "scrape-file-failed",
    (e) => {
      const { subject_id, relative_path, error } = e.payload;
      if (!isPipelineFile(relative_path)) return;
      pipeline().touch(relative_path, subject_id, {
        download: "error",
        error,
        errorKind: undefined,
        errorRetryable: undefined,
        errorLatching: undefined,
      });
    },
  );
  useTauriEvent<{ subject_id: number; relative_path: string; size_bytes: number; category: string | null; canvas_id: number | null; source_url: string | null; source_created_at: string | null; source_updated_at: string | null; action: SyncFileAction | "removed" }>(
    "scrape-file",
    async (e) => {
      const { subject_id, relative_path, size_bytes, category, canvas_id, source_url, source_created_at, source_updated_at, action } = e.payload;
      // A file the sync deleted from disk (Ed dropped it from a challenge's
      // workspace) loses its row; one that never had a row is fine.
      if (action === "removed") {
        try {
          await parseStatuses.mutate(subject_id, relative_path, async () => {
            const row = await getFileByRelativePath(relative_path);
            if (!row) return;
            await deleteFileRow(row.id);
            forgetPageSnapshots(row.id);
          });
        } catch (err) {
          addLog(`${relative_path}: its row was not removed: ${err}`, "warning").catch(() => {});
        }
        window.dispatchEvent(
          new CustomEvent<FileScraped>(FILE_SCRAPED_EVENT, { detail: { subject_id, canvas_id } }),
        );
        return;
      }
      if (isPipelineFile(relative_path)) {
        if (action === "unchanged") {
          pipeline().confirmDownload(relative_path);
        } else if (action === "updated") {
          // New bytes: Rust purged the parse artifacts, so both stages start over.
          pipeline().touch(relative_path, subject_id, {
            download: "done",
            downloadedAt: Date.now(),
            parse: "pending",
            embed: "pending",
            pagesDone: 0,
            totalPages: 0,
            embedPagesDone: 0,
            embedTotalPages: 0,
            parseQueuePos: undefined,
            parsedAt: undefined,
            embeddedAt: undefined,
            skippedAt: undefined,
            ...NO_UPLOAD,
            ...NO_ERROR,
          });
        } else {
          pipeline().touch(relative_path, subject_id, {
            download: "done",
            downloadedAt: Date.now(),
          });
        }
      }
      const filename = relative_path.split("/").pop() ?? relative_path;
      const ext = filename.includes(".") ? filename.split(".").pop()! : "md";
      try {
        await parseStatuses.mutate(subject_id, relative_path, async () => {
          await upsertFile(subject_id, filename, relative_path, ext, size_bytes, category ?? undefined, canvas_id ?? undefined, source_url ?? undefined, source_created_at, source_updated_at);
          if (action === "new" || action === "updated") {
            await markFileContentChanged(subject_id, relative_path);
          }
          // Rust purged the parse artifacts; clear stale statuses and pages
          // so search never serves the old text. A spreadsheet's are replaced
          // by Rust as it converts, which may already have happened.
          if (action === "updated" && isPdfBacked(relative_path)) {
            await resetFilePipeline(subject_id, relative_path);
          }
        });
      } catch { /* ignore */ }
      window.dispatchEvent(
        new CustomEvent<FileScraped>(FILE_SCRAPED_EVENT, { detail: { subject_id, canvas_id } }),
      );
      // Sync History ledger, only for a run started from this instance.
      const runId = useSyncStore.getState().runId;
      if (runId != null) {
        addSyncRunFile(runId, subject_id, relative_path, action ?? "new", size_bytes).catch(() => {});
      }
    },
  );
  useTauriEvent<{ level: string; course: string; message: string }>("scrape-log", (e) => {
    const { level, message } = e.payload;
    const mapped = level === "error" ? "error" : level === "warning" ? "warning" : "info";
    addLog(message, mapped).catch(() => {});
  });
  useTauriEvent<{ count: number; cancelled?: boolean }>("scrape-complete", async (e) => {
    const { runId, subjects } = useSyncStore.getState();
    // Stores first: a failed DB write must not leave the run looking live.
    useSyncStore.getState().complete(e.payload.count, !!e.payload.cancelled);
    pipeline().failStalledDownloads();
    if (runId != null) {
      try {
        await finishSyncRun(runId, "completed", e.payload.count, e.payload.count);
        await addLog(`Synced ${e.payload.count} subject(s)`);
      } catch (err) {
        console.error("recording the finished sync run failed", err);
      }
    }
    // Refresh Echo360 lecture lists (metadata only) and calendar events;
    // a per-subject failure just logs.
    if (!e.payload.cancelled && subjects.length > 0) {
      const { lectures, calendar } = await getSyncOptions();
      if (lectures) {
        for (const s of subjects) {
          try {
            const data = await invoke<LectureData[]>("echo360_sync_lectures", {
              canvasCourseId: s.id,
            });
            await upsertLectures(s.id, data);
          } catch (err) {
            addLog(`lectures ${s.code}: ${err}`, "warning").catch(() => {});
          }
        }
      }
      if (calendar) {
        for (const s of subjects) {
          try {
            await syncCalendar(s.id);
          } catch (err) {
            addLog(`calendar ${s.code}: ${err}`, "warning").catch(() => {});
          }
        }
        window.dispatchEvent(new CustomEvent(CALENDAR_UPDATED_EVENT));
      }
    }
  });
  useTauriEvent<string>("scrape-error", async (e) => {
    const runId = useSyncStore.getState().runId;
    useSyncStore.getState().fail(typeof e.payload === "string" ? e.payload : "Sync error");
    pipeline().failStalledDownloads();
    if (runId != null) {
      await finishSyncRun(runId, "failed", 0, 0, e.payload).catch((err) =>
        console.error("recording the failed sync run failed", err),
      );
    }
  });
}
