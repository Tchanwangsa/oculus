import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  setParseStatus, setEmbedStatus, upsertFile, finishSyncRun, addLog,
  addSyncRunFile, getSyncOptions, markFileContentChanged, resetFilePipeline,
  upsertLectures, replaceCalendarEvents, getFileByRelativePath,
  type CalendarEventData, type LectureData, type SyncFileAction,
} from "@/lib/db";
import { useSyncStore } from "@/stores/syncStore";
import { useParseStore } from "@/stores/parseStore";
import { usePipelineStore } from "@/stores/pipelineStore";
import { reportEmbedPages, useIndexStore } from "@/stores/indexStore";
import { embedReady } from "@/lib/retrieval";
import type { SyncProgress } from "@/stores/syncStore";
import type { ParseJob } from "@/stores/parseStore";
import { isPdfBacked } from "@/lib/fileTypes";
import { CALENDAR_UPDATED_EVENT } from "@/lib/calendar";
import { notifyProjectsUpdated } from "@/lib/projects";
import { useHarnessStore } from "@/stores/harnessStore";
import type { HarnessEnvelope } from "@/lib/harness";

/**
 * The whole `parse-status` vocabulary Rust emits, and the whole vocabulary
 * `files.parse_status` stores. `"quality"` is the terminal success — the name
 * outlived the tier it was named after (see `parseStore`), and renaming it
 * would invalidate every already-parsed row in the library.
 */
const PARSE_STATUSES = new Set(["queued", "running", "quality", "error"]);

/**
 * The `embed-status` vocabulary, which is the parse one with the historical
 * name taken out: the terminal success is `"done"`, because unlike
 * `'quality'` it was never written into a library's worth of rows.
 */
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

    // Is there an embedder behind the third stage? Asked once here, at the one
    // place that is mounted for the life of the app, and re-asked by Settings
    // → Library whenever a key is saved or cleared. Nothing is queued
    // automatically while this is false — see `indexStore`.
    void embedReady().then((ready) => useIndexStore.getState().setReady(ready));

    // ── CLI agents ──────────────────────────────────────────────────────────
    // App-level, not page-level: a thread keeps running while you are on
    // another page, and the sidebar's spinner needs to know.
    //
    // The second half of this listener is the one hop that keeps an open board
    // honest. `oculus project` / `oculus task` write the same tables the board
    // reads, but from a *separate process* with its own connection — nothing
    // in this webview's pool notices a row the agent wrote. So when a tool call
    // whose command names a project or a task finishes, the board is told to
    // re-read. `tool_finished` carries only the call's id, not its command, so
    // the ids worth watching are remembered from `tool_started`.
    //
    // The test is the command text alone, not `kind === "oculus_cli"`:
    // `is_oculus_cli` in app/src-tauri/src/harness/event.rs only word-matches
    // the first few words, so `cd … && oculus task add` classifies as plain
    // Bash and a kind gate would drop exactly the write this hop exists for.
    // The trade is deliberate and one-sided — a false positive (an agent
    // grepping for the words "oculus task") costs one re-read of a handful of
    // rows, a false negative costs a board that is silently wrong.
    const planningCalls = new Set<string>();
    const WRITES_PLANNING = /\boculus\s+(project|task)\b/;
    unsubs.push(
      listen<HarnessEnvelope>("harness-event", (e) => {
        useHarnessStore.getState().apply(e.payload);
        const ev = e.payload.event;
        if (ev.type === "tool_started") {
          // `title` is the whole command for a Bash-shaped tool, which is what
          // makes matching on it possible at all.
          if (WRITES_PLANNING.test(ev.title)) {
            planningCalls.add(`${e.payload.threadId}:${ev.id}`);
          }
        } else if (ev.type === "tool_finished") {
          // Regardless of `ok`, for the same reason: a failed write is atomic
          // and changes nothing, but a command that timed out may still have
          // landed.
          if (planningCalls.delete(`${e.payload.threadId}:${ev.id}`)) {
            notifyProjectsUpdated();
          }
        }
      }),
    );

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

    // ── PDF parse stage events ──────────────────────────────────────────────
    // One stage, emitted by Rust directly (there is no loopback IPC server and
    // no sidecar any more). `"quality"` is the terminal success; see
    // `PARSE_STATUSES` above for why that name stayed.
    unsubs.push(
      listen<ParseJob>("parse-status", async (e) => {
        const ev = e.payload;
        const path = ev.relative_path;
        if (!path) return;

        // A progress heartbeat can race the completion notify by a tick; a
        // "running" arriving after the parse finished must not undo it.
        const staleRunning =
          ev.status === "running" &&
          usePipelineStore.getState().items[path]?.parse === "done";
        if (staleRunning) return;

        if (PARSE_STATUSES.has(ev.status)) {
          // `update` also records the failure's discriminants and raises or
          // lifts the latch the background sweep reads.
          useParseStore.getState().update(ev);
          try {
            await setParseStatus(ev.subject_id, path, ev.status);
          } catch { /* ignore */ }
        }

        // Pipeline table: every status advances the one parse stage.
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
            // The third stage, and the one hop that makes the pipeline
            // continuous: a file that has just become parseable text is also a
            // file that can be embedded, and the queue is serial, so appending
            // here costs nothing but a place in line. Gated on a stored Voyage
            // key — with none there is no backend to queue against, and the
            // stage is not drawn at all.
            //
            // The file row is read rather than assumed because the queue needs
            // its `id`: `pages` is keyed on `file_id`, and this event carries
            // a path. `scrape-file` has already upserted it by the time a
            // parse can have finished.
            if (useIndexStore.getState().ready) {
              getFileByRelativePath(path)
                .then((file) => {
                  if (file) useIndexStore.getState().enqueueFile(file);
                })
                .catch(() => {});
            }
            break;
          case "error":
            // The discriminants ride into the row rather than being dropped:
            // the failure UI has to be able to say whether a retry is worth
            // offering at all, and that is not knowable from the message.
            touch(path, ev.subject_id, {
              parse: "error",
              error: ev.error ?? "Parse failed",
              errorKind: ev.kind,
              errorRetryable: ev.retryable,
              errorLatching: ev.latching,
            });
            break;
        }
      }),
    );

    // ── Page embedding stage events ─────────────────────────────────────────
    // The third stage, and the only one that reports progress *inside* a
    // document on a clock measured in minutes per page. Same shape as
    // `parse-status` deliberately — see `app/src-tauri/src/embed/events.rs`.
    unsubs.push(
      listen<EmbedJob>("embed-status", async (e) => {
        const ev = e.payload;
        const path = ev.relative_path;
        if (!path || !EMBED_STATUSES.has(ev.status)) return;

        // A progress heartbeat can race the completion notify by a tick; a
        // "running" arriving after the file finished must not undo it.
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
            // The settings page's bar is drawn from the run, and this is the
            // only place the page inside the current document is known.
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

        // Rust writes `'done'` itself when the ingest commits. What it cannot
        // write is a failure — the transaction never ran — and a row that
        // forgot its failure on restart would come back as merely waiting.
        if (ev.status === "done" || ev.status === "error") {
          try {
            await setEmbedStatus(ev.subject_id, path, ev.status);
          } catch { /* ignore */ }
        }
      }),
    );

    return () => {
      unsubs.forEach((u) => u.then((f) => f()).catch(() => {}));
    };
  }, []);
}
