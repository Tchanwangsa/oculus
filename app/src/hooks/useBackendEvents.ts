import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  setParseStatus, upsertFile, finishSyncRun, addLog,
  addSyncRunFile, getSyncOptions, markFileContentChanged, resetFilePipeline,
  upsertLectures, replaceCalendarEvents,
  type CalendarEventData, type LectureData, type SyncFileAction,
} from "@/lib/db";
import { useSyncStore } from "@/stores/syncStore";
import { useParseStore } from "@/stores/parseStore";
import { usePipelineStore } from "@/stores/pipelineStore";
import type { SyncProgress } from "@/stores/syncStore";
import type { ParseJob } from "@/stores/parseStore";
import { embedFile } from "@/lib/retrieval";
import { runEventAutomations } from "@/lib/automations";
import { isPdfBacked } from "@/lib/fileTypes";
import { CALENDAR_UPDATED_EVENT } from "@/lib/calendar";
import { getDb } from "@/lib/db";

/**
 * Statuses the sidecar always sent, which the file-row badges and the DB's
 * `files.parse_status` column understand. Newer stage signals ("parsing",
 * "embedding", "embedded", "embed_error") exist only for the pipeline view and
 * must not be persisted or fed to the badge store.
 */
const PARSE_STATUSES = new Set(["fast", "quality", "queued", "running", "error"]);

/**
 * Index a PDF right after it parses.
 *
 * Fire-and-forget: a failed embed must never block or fail the parse flow, and
 * `embedPending()` will retry it later. Serialised through one promise chain
 * because the sidecar holds a single model — firing these in parallel would
 * queue on the GPU anyway.
 */
let embedChain: Promise<unknown> = Promise.resolve();

async function embedAfterParse(subjectId: number, relativePath: string) {
  if (!isPdfBacked(relativePath)) return;
  embedChain = embedChain.then(async () => {
    try {
      const db = await getDb();
      const rows = await db.select<{ id: number }[]>(
        `SELECT id FROM files WHERE subject_id = $1 AND relative_path = $2`,
        [subjectId, relativePath],
      );
      const fileId = rows[0]?.id;
      if (fileId == null) return;
      await embedFile(fileId, relativePath);
    } catch (e) {
      console.error("embed after parse failed", relativePath, e);
    }
  });
}

const isPipelinePdf = (path: string) => isPdfBacked(path);

/**
 * Single app-level bridge: subscribes to all backend Tauri events and writes
 * them into the global stores (and the DB). Mount ONCE near the app root so
 * progress survives page navigation. UI reads from the stores, never listens
 * directly.
 */
export function useBackendEvents() {
  useEffect(() => {
    const unsubs: Array<Promise<() => void>> = [];
    const pipeline = () => usePipelineStore.getState();

    // ── Sync (scrape) events ────────────────────────────────────────────────
    unsubs.push(
      listen<SyncProgress>("scrape-progress", (e) => {
        if (e.payload.phase === "complete") return;
        useSyncStore.getState().setProgress(e.payload);
      }),
    );
    unsubs.push(
      listen<{ subject_id: number; relative_path: string; filename: string }>(
        "scrape-file-start",
        (e) => {
          const { subject_id, relative_path } = e.payload;
          if (!isPipelinePdf(relative_path)) return;
          pipeline().touch(relative_path, subject_id, { download: "active" });
        },
      ),
    );
    unsubs.push(
      listen<{ subject_id: number; relative_path: string; size_bytes: number; category: string | null; canvas_id: number | null; source_url: string | null; action: SyncFileAction }>(
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
            // Changed bytes: the Rust side purged the on-disk parse artifacts;
            // clear the DB's stale statuses and stored pages so the pipeline
            // re-runs and search never serves the old text.
            if (action === "updated" && isPdfBacked(relative_path)) {
              await resetFilePipeline(subject_id, relative_path);
            }
          } catch { /* ignore */ }
          // Per-run ledger, feeds the Sync History table. Only recorded while
          // a run started from this app instance is live.
          const runId = useSyncStore.getState().runId;
          if (runId != null) {
            addSyncRunFile(runId, subject_id, relative_path, action ?? "new", size_bytes).catch(() => {});
          }
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
        const { runId, subjects } = useSyncStore.getState();
        if (runId != null) {
          await finishSyncRun(runId, "completed", e.payload.count, e.payload.count);
          await addLog(`Synced ${e.payload.count} subject(s)`);
        }
        useSyncStore.getState().complete(e.payload.count, !!e.payload.cancelled);
        // Anything still "downloading" now will never finish — the run is over.
        pipeline().failStalledDownloads();
        // Refresh each subject's Echo360 lecture *list* (metadata only, no
        // video downloads) — same call the Lectures tab's button makes. A
        // course without Echo, or a failed LTI launch, just logs and moves on.
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
          // Class times and due dates, from Canvas's calendar API. Same shape
          // as the lecture refresh above: a post-scrape pass the frontend
          // drives, since nothing about it lands on disk.
          if (calendar) {
            for (const s of subjects) {
              try {
                const rows = await invoke<CalendarEventData[]>("calendar_sync_events", {
                  canvasCourseId: s.id,
                });
                await replaceCalendarEvents(s.id, rows);
              } catch (err) {
                addLog(`calendar ${s.code}: ${err}`, "warning").catch(() => {});
              }
            }
            window.dispatchEvent(new CustomEvent(CALENDAR_UPDATED_EVENT));
          }
        }

        // Event-triggered automations. Fire-and-forget: a digest waits on
        // parses for minutes, and nothing here should hold up the sync's
        // completion path. A cancelled run has no meaningful file set.
        if (!e.payload.cancelled && runId != null) {
          runEventAutomations("sync-complete", { runId }).catch((err) =>
            addLog(`automation: ${err}`, "warning").catch(() => {}),
          );
        }
      }),
    );
    unsubs.push(
      listen<string>("scrape-error", async (e) => {
        const runId = useSyncStore.getState().runId;
        if (runId != null) await finishSyncRun(runId, "failed", 0, 0, e.payload);
        useSyncStore.getState().fail(typeof e.payload === "string" ? e.payload : "Sync error");
        pipeline().failStalledDownloads();
      }),
    );

    // ── PDF parse + embed stage events ──────────────────────────────────────
    unsubs.push(
      listen<ParseJob>("parse-status", async (e) => {
        const ev = e.payload;
        const path = ev.relative_path;
        if (!path) return;

        // The sidecar's progress heartbeat can race the completion notify by
        // a tick; a "running" arriving after quality finished must not undo it.
        const staleRunning =
          ev.status === "running" &&
          usePipelineStore.getState().items[path]?.quality === "done";
        if (staleRunning) return;

        if (PARSE_STATUSES.has(ev.status)) {
          useParseStore.getState().update(ev);
          try {
            await setParseStatus(ev.subject_id, path, ev.status);
          } catch { /* ignore */ }
        }

        // Parsed pages are only useful once they are searchable, so indexing
        // follows parsing automatically. `quality` overwrites the markdown the
        // `fast` pass wrote, so re-embedding then refreshes the stored text —
        // the vectors are unchanged (they come from the page image) but the
        // upsert picks up the better markdown.
        if (ev.status === "fast" || ev.status === "quality") {
          void embedAfterParse(ev.subject_id, path);
        }

        // Pipeline table: every status advances exactly one stage.
        const touch = pipeline().touch;
        switch (ev.status) {
          case "parsing":
            touch(path, ev.subject_id, { download: "done", fast: "active" });
            break;
          case "fast":
            touch(path, ev.subject_id, { download: "done", fast: "done", fastParsedAt: Date.now() });
            break;
          case "queued":
            touch(path, ev.subject_id, {
              download: "done",
              fast: "done",
              quality: "queued",
              qualityQueuePos: ev.position,
            });
            break;
          case "running":
            touch(path, ev.subject_id, {
              download: "done",
              fast: "done",
              quality: "active",
              pagesDone: ev.pages_done ?? 0,
              totalPages: ev.total_pages ?? 0,
              qualityQueuePos: undefined,
            });
            break;
          case "quality":
            touch(path, ev.subject_id, {
              download: "done",
              fast: "done",
              quality: "done",
              parsedAt: Date.now(),
            });
            break;
          case "error":
            touch(path, ev.subject_id, { quality: "error", error: ev.error ?? "Parse failed" });
            break;
          case "embedding":
            touch(path, ev.subject_id, {
              embed: "active",
              embedPagesDone: ev.pages_done ?? 0,
              embedTotalPages: ev.total_pages ?? 0,
            });
            break;
          case "embedded": {
            // The embed after the fast parse is provisional — the real finish
            // line is the embed that follows the quality parse.
            const item = usePipelineStore.getState().items[path];
            const final = item?.quality === "done";
            touch(path, ev.subject_id, {
              embed: final ? "done" : "pending",
              ...(final ? { embeddedAt: Date.now() } : {}),
            });
            break;
          }
          case "embed_error": {
            const item = usePipelineStore.getState().items[path];
            // Before quality is done another embed attempt is still coming,
            // so only the final one gets to mark the stage failed.
            if (item?.quality === "done") {
              touch(path, ev.subject_id, { embed: "error", error: ev.error ?? "Embed failed" });
            } else {
              touch(path, ev.subject_id, { embed: "pending" });
            }
            break;
          }
        }
      }),
    );

    return () => {
      unsubs.forEach((u) => u.then((f) => f()));
    };
  }, []);
}
