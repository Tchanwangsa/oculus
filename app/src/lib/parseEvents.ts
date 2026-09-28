import { setParseStatus } from "@/lib/db";
import { useParseStore, type ParseJob } from "@/stores/parseStore";
import { usePipelineStore } from "@/stores/pipelineStore";
import type { SyncWriteQueue } from "@/lib/syncWrites";

/** Rust has one parse stage. The historical quality success is retained in
 * the database; embedding reports its own independent embed-status event. */
const PARSE_STATUSES = new Set(["quality", "queued", "running", "error"]);

/** Keep stale-heartbeat checks, commits and visible progress in one queue,
 * after scrape-file inserts. A completed parse must never embed a missing row. */
export function queueParseEvent(
  writes: SyncWriteQueue,
  runId: number | null,
  ev: ParseJob,
  onParsed: (subjectId: number, relativePath: string) => void,
  onError: (message: string) => void,
): Promise<boolean> {
  const path = ev.relative_path;
  if (!path || !PARSE_STATUSES.has(ev.status)) return Promise.resolve(false);
  return writes.enqueue(runId, `Saving parse status for ${path}`, async () => {
    if (ev.status === "running" && usePipelineStore.getState().items[path]?.parse === "done") return;
    await setParseStatus(ev.subject_id, path, ev.status);
    useParseStore.getState().update(ev);
    const touch = usePipelineStore.getState().touch;
    switch (ev.status) {
      case "queued":
        touch(path, ev.subject_id, {
          download: "done", parse: "queued", parseQueuePos: ev.position,
        });
        break;
      case "running":
        touch(path, ev.subject_id, {
          download: "done", parse: "active", pagesDone: ev.pages_done ?? 0,
          totalPages: ev.total_pages ?? 0, parseQueuePos: undefined,
        });
        break;
      case "quality":
        touch(path, ev.subject_id, {
          download: "done", parse: "done", parsedAt: Date.now(),
          error: undefined, errorKind: undefined,
          errorRetryable: undefined, errorLatching: undefined,
        });
        onParsed(ev.subject_id, path);
        break;
      case "error":
        touch(path, ev.subject_id, {
          parse: "error", error: ev.error ?? "Parse failed", errorKind: ev.kind,
          errorRetryable: ev.retryable, errorLatching: ev.latching,
        });
        break;
    }
  }, (message) => {
    useParseStore.getState().update({ ...ev, status: "error", error: message });
    usePipelineStore.getState().touch(path, ev.subject_id, { parse: "error", error: message });
    onError(message);
  });
}
