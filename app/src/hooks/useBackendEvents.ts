import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  setParseStatus, setEmbedStatus, upsertFile, finishSyncRun, addLog,
  addSyncRunFile, getSyncOptions, markFileContentChanged, resetFilePipeline,
  upsertLectures, getFileByRelativePath,
  type LectureData, type SyncFileAction,
} from "@/lib/db";
import { useSyncStore } from "@/stores/syncStore";
import { useParseStore } from "@/stores/parseStore";
import { usePipelineStore } from "@/stores/pipelineStore";
import { reportEmbedPages, useIndexStore } from "@/stores/indexStore";
import { embedReady } from "@/lib/retrieval";
import type { SyncProgress } from "@/stores/syncStore";
import type { ParseJob } from "@/stores/parseStore";
import { isPdfBacked } from "@/lib/fileTypes";
import { CALENDAR_UPDATED_EVENT, syncCalendar } from "@/lib/calendar";
import { FILE_SCRAPED_EVENT, type FileScraped } from "@/lib/syncRunner";
import { notifyProjectsUpdated } from "@/lib/projects";
import { useHarnessStore } from "@/stores/harnessStore";
import type { HarnessEnvelope } from "@/lib/harness";
import { useTauriEvent } from "@/hooks/useEvents";

/** `parse-status` and `files.parse_status` share this vocabulary; `"quality"`
 *  is a finished parse, and renaming it would invalidate every stored row. */
const PARSE_STATUSES = new Set(["queued", "running", "quality", "error"]);

const EMBED_STATUSES = new Set(["queued", "running", "done", "error"]);

/** `embed-status`, exactly as `app/src-tauri/src/embed/events.rs` emits it. */
interface EmbedJob {
  relative_path: string;
  subject_id: number;
  status: string;
  pages_done?: number;
  total_pages?: number;
  error?: string;
  kind?: string;
  retryable?: boolean;
  latching?: boolean;
}

const isPipelinePdf = (path: string) => isPdfBacked(path);
const pipeline = () => usePipelineStore.getState();
const WRITES_PLANNING = /\boculus\s+(project|task)\b/;

/**
 * The one bridge from backend Tauri events into the global stores and the DB.
 * Mount once at the app root so progress survives navigation; UI reads the
 * stores and never listens directly.
 */
export function useBackendEvents() {
  // Re-asked by Settings → Library when a key is saved or cleared.
  useEffect(() => {
    void embedReady().then((ready) => useIndexStore.getState().setReady(ready));
  }, []);

  // ── CLI agents ──────────────────────────────────────────────────────────
  // An agent's `oculus project`/`oculus task` writes from another process,
  // so the board is told to re-read when such a call finishes (ids are
  // remembered from `tool_started`; `tool_finished` has no command). Match
  // the command text, not `kind === "oculus_cli"`: `is_oculus_cli`
  // (`harness/event.rs`) misses `cd … && oculus task add`.
  const planningCalls = useRef(new Set<string>()).current;
  useTauriEvent<HarnessEnvelope>("harness-event", (e) => {
    useHarnessStore.getState().apply(e.payload);
    const ev = e.payload.event;
    if (ev.type === "tool_started") {
      // `title` is the whole command for a Bash-shaped tool.
      if (WRITES_PLANNING.test(ev.title)) {
        planningCalls.add(`${e.payload.threadId}:${ev.id}`);
      }
    } else if (ev.type === "tool_finished") {
      // Regardless of `ok`: a timed-out command may still have landed.
      if (planningCalls.delete(`${e.payload.threadId}:${ev.id}`)) {
        notifyProjectsUpdated();
      }
    }
  });

  // ── Sync (scrape) events ────────────────────────────────────────────────
  useTauriEvent<SyncProgress>("scrape-progress", (e) => {
    if (e.payload.phase === "complete") return;
    useSyncStore.getState().setProgress(e.payload);
  });
  useTauriEvent<{ subject_id: number; relative_path: string; filename: string }>(
    "scrape-file-start",
    (e) => {
      const { subject_id, relative_path } = e.payload;
      if (!isPipelinePdf(relative_path)) return;
      pipeline().touch(relative_path, subject_id, { download: "active" });
    },
  );
  useTauriEvent<{ subject_id: number; relative_path: string; size_bytes: number; category: string | null; canvas_id: number | null; source_url: string | null; action: SyncFileAction }>(
    "scrape-file",
    async (e) => {
      const { subject_id, relative_path, size_bytes, category, canvas_id, source_url, action } = e.payload;
      if (isPipelinePdf(relative_path)) {
        pipeline().touch(relative_path, subject_id, {
          download: "done",
          downloadedAt: Date.now(),
        });
      }
      const filename = relative_path.split("/").pop() ?? relative_path;
      const ext = filename.includes(".") ? filename.split(".").pop()! : "md";
      try {
        await upsertFile(subject_id, filename, relative_path, ext, size_bytes, category ?? undefined, canvas_id ?? undefined, source_url ?? undefined);
        if (action === "new" || action === "updated") {
          await markFileContentChanged(subject_id, relative_path);
        }
        // Rust purged the parse artifacts; clear stale statuses and pages
        // so search never serves the old text.
        if (action === "updated" && isPdfBacked(relative_path)) {
          await resetFilePipeline(subject_id, relative_path);
        }
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
    if (runId != null) {
      await finishSyncRun(runId, "completed", e.payload.count, e.payload.count);
      await addLog(`Synced ${e.payload.count} subject(s)`);
    }
    useSyncStore.getState().complete(e.payload.count, !!e.payload.cancelled);
    pipeline().failStalledDownloads();
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
    if (runId != null) await finishSyncRun(runId, "failed", 0, 0, e.payload);
    useSyncStore.getState().fail(typeof e.payload === "string" ? e.payload : "Sync error");
    pipeline().failStalledDownloads();
  });

  // ── PDF parse stage events ──────────────────────────────────────────────
  useTauriEvent<ParseJob>("parse-status", async (e) => {
    const ev = e.payload;
    const path = ev.relative_path;
    if (!path) return;

    // A late "running" heartbeat must not undo a finished parse.
    const staleRunning =
      ev.status === "running" &&
      usePipelineStore.getState().items[path]?.parse === "done";
    if (staleRunning) return;

    if (PARSE_STATUSES.has(ev.status)) {
      // Also records failure discriminants and the sweep's latch.
      useParseStore.getState().update(ev);
      try {
        await setParseStatus(ev.subject_id, path, ev.status);
      } catch { /* ignore */ }
    }

    const touch = pipeline().touch;
    switch (ev.status) {
      case "queued":
        touch(path, ev.subject_id, {
          download: "done",
          parse: "queued",
          parseQueuePos: ev.position,
        });
        break;
      case "running":
        touch(path, ev.subject_id, {
          download: "done",
          parse: "active",
          pagesDone: ev.pages_done ?? 0,
          totalPages: ev.total_pages ?? 0,
          parseQueuePos: undefined,
        });
        break;
      case "quality":
        touch(path, ev.subject_id, {
          download: "done",
          parse: "done",
          parsedAt: Date.now(),
          error: undefined,
          errorKind: undefined,
          errorRetryable: undefined,
          errorLatching: undefined,
        });
        // Auto-embed (gated on a Voyage key). The queue needs the file
        // row's `id`, and this event carries only a path.
        if (useIndexStore.getState().ready) {
          getFileByRelativePath(path)
            .then((file) => {
              if (file) useIndexStore.getState().enqueueFile(file);
            })
            .catch(() => {});
        }
        break;
      case "error":
        touch(path, ev.subject_id, {
          parse: "error",
          error: ev.error ?? "Parse failed",
          errorKind: ev.kind,
          errorRetryable: ev.retryable,
          errorLatching: ev.latching,
        });
        break;
    }
  });

  // ── Page embedding stage events ─────────────────────────────────────────
  // Same shape as `parse-status` on purpose (`embed/events.rs`).
  useTauriEvent<EmbedJob>("embed-status", async (e) => {
    const ev = e.payload;
    const path = ev.relative_path;
    if (!path || !EMBED_STATUSES.has(ev.status)) return;

    // A late "running" heartbeat must not undo a finished embed.
    if (ev.status === "running" && pipeline().items[path]?.embed === "done") return;

    const touch = pipeline().touch;
    switch (ev.status) {
      case "queued":
        touch(path, ev.subject_id, { parse: "done", embed: "queued" });
        break;
      case "running":
        touch(path, ev.subject_id, {
          parse: "done",
          embed: "active",
          embedPagesDone: ev.pages_done ?? 0,
          embedTotalPages: ev.total_pages ?? 0,
        });
        reportEmbedPages(path, ev.pages_done ?? 0, ev.total_pages ?? 0);
        break;
      case "done":
        touch(path, ev.subject_id, {
          parse: "done",
          embed: "done",
          embedPagesDone: ev.pages_done ?? 0,
          embedTotalPages: ev.total_pages ?? 0,
          embeddedAt: Date.now(),
          error: undefined,
          errorKind: undefined,
          errorRetryable: undefined,
          errorLatching: undefined,
        });
        break;
      case "error":
        touch(path, ev.subject_id, {
          embed: "error",
          error: ev.error ?? "Embedding failed",
          errorKind: ev.kind,
          errorRetryable: ev.retryable,
          errorLatching: ev.latching,
        });
        break;
    }

    // Rust persists `done` on commit but cannot persist a failure (the
    // transaction never ran); without this it reads as waiting on restart.
    if (ev.status === "done" || ev.status === "error") {
      try {
        await setEmbedStatus(ev.subject_id, path, ev.status);
      } catch { /* ignore */ }
    }
  });
}
