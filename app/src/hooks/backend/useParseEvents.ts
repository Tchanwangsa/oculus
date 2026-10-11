import { getFileByRelativePath } from "@/lib/db";
import { useParseStore } from "@/stores/sync/parseStore";
import { NO_UPLOAD, runningPatch, usePipelineStore } from "@/stores/sync/pipelineStore";
import { useIndexStore } from "@/stores/sync/indexStore";
import type { ParseJob } from "@/stores/sync/parseStore";
import { isPdfBacked } from "@/lib/files/fileTypes";
import { useTauriEvent } from "@/hooks/backend/useEvents";
import { NO_ERROR, PARSE_STATUSES, clearErrorUnless, parseStatuses, pipeline } from "./pipelineEvents";

/** PDF parse stage events. */
export function useParseEvents() {
  useTauriEvent<ParseJob>("parse-status", (e) => {
    const ev = e.payload;
    const path = ev.relative_path;
    if (!path) return;

    // A late "running" heartbeat must not undo a finished parse, nor a
    // heartbeat or the cancelled parse's error undo a skip.
    const parse = usePipelineStore.getState().items[path]?.parse;
    const skipped =
      parse === "skipped" || useParseStore.getState().statuses[path] === "skipped";
    const stale =
      (ev.status === "running" && (parse === "done" || skipped)) ||
      (ev.status === "error" && skipped);
    if (stale) return;

    let persisted = Promise.resolve();
    if (PARSE_STATUSES.has(ev.status)) {
      // Also records failure discriminants and the sweep's latch.
      useParseStore.getState().update(ev);
      // Persistence is ordered per file, while live progress lands immediately.
      persisted = parseStatuses.write(ev.subject_id, path, ev.status).catch(() => {});
    }

    const touch = pipeline().touch;
    switch (ev.status) {
      case "queued":
        touch(path, ev.subject_id, {
          download: "done",
          parse: "queued",
          parseQueuePos: ev.position,
          skippedAt: undefined,
          ...NO_UPLOAD,
        });
        break;
      case "running":
        touch(path, ev.subject_id, {
          download: "done",
          parse: "active",
          pagesDone: ev.pages_done ?? 0,
          totalPages: ev.total_pages ?? 0,
          parseQueuePos: undefined,
          ...runningPatch(pipeline().items[path], ev, Date.now()),
        });
        break;
      case "quality": {
        // Rust's skip path (already parsed) emits this too; that is no new
        // parse, and an embedded file needs no re-queue.
        const prev = pipeline().items[path];
        touch(path, ev.subject_id, {
          download: "done",
          parse: "done",
          parsePhase: undefined,
          ...(prev?.parse !== "done" ? { parsedAt: Date.now() } : {}),
          ...clearErrorUnless(path, "embed"),
        });
        // Auto-embed (gated on a Voyage key; a spreadsheet's text is never
        // embedded). The queue needs the file row's `id`, and this event
        // carries only a path.
        if (useIndexStore.getState().ready && prev?.embed !== "done" && isPdfBacked(path)) {
          persisted.then(() => getFileByRelativePath(path))
            .then((file) => {
              if (file) useIndexStore.getState().enqueueFile(file);
            })
            .catch(() => {});
        }
        break;
      }
      // The user's skip: settled, so no error, latch or queue position.
      case "skipped":
        touch(path, ev.subject_id, {
          parse: "skipped",
          parsePhase: undefined,
          parseQueuePos: undefined,
          skippedAt: Date.now(),
          ...NO_ERROR,
        });
        break;
      case "error":
        touch(path, ev.subject_id, {
          parse: "error",
          parsePhase: undefined,
          error: ev.error ?? "Parse failed",
          errorKind: ev.kind,
          errorRetryable: ev.retryable,
          errorLatching: ev.latching,
        });
        break;
    }
  });
}
