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
import { embedFile } from "@/lib/retrieval";
import { getDb } from "@/lib/db";

/** How long a finished job stays visible in the activity panel. */
const COMPLETED_LINGER_MS = 2500;

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
  if (!relativePath.toLowerCase().endsWith(".pdf")) return;
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

/**
 * Single app-level bridge: subscribes to all backend Tauri events and writes
 * them into the global stores (and the DB). Mount ONCE near the app root so
 * progress survives page navigation. UI reads from the stores, never listens
 * directly.
 */
export function useBackendEvents() {
  // PDF parse aggregate tracking. Reset once a batch finishes, so the next
  // batch starts counting from 0/0 instead of inheriting stale totals.
  const pdfSeen = useRef(new Set<string>());
  const pdfCompleted = useRef(new Set<string>());
  const pdfClearTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

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

    // ── Lecture download events ─────────────────────────────────────────────
    unsubs.push(
      listen<{ mediaId: string; percent: number; phase: string }>(
        "lecture-download-progress",
        (e) => {
          const { mediaId, percent, phase } = e.payload;
          const id = `lecture_download:${mediaId}`;
          if (phase === "complete" || phase === "error") {
            useJobStore.getState().remove(id);
            return;
          }
          useJobStore.getState().upsert({
            id,
            type: "lecture_download",
            status: "running",
            progress_current: Math.round(percent),
            progress_total: 100,
          });
        },
      ),
    );

    // ── PDF parse events ────────────────────────────────────────────────────
    unsubs.push(
      listen<ParseJob>("parse-status", async (e) => {
        const ev = e.payload;
        useParseStore.getState().update(ev);
        try {
          await setParseStatus(ev.subject_id, ev.relative_path, ev.status);
        } catch { /* ignore */ }

        // Parsed pages are only useful once they are searchable, so indexing
        // follows parsing automatically. `quality` overwrites the markdown the
        // `fast` pass wrote, so re-embedding then refreshes the stored text —
        // the vectors are unchanged (they come from the page image) but the
        // upsert picks up the better markdown.
        if (ev.status === "fast" || ev.status === "quality") {
          void embedAfterParse(ev.subject_id, ev.relative_path);
        }

        // Aggregate into jobStore
        const path = ev.relative_path;
        const filename = path.split("/").pop() ?? path;
        const isTerminal = ev.status === "fast" || ev.status === "quality" || ev.status === "error";

        // A new file arriving after a batch "completed" starts a fresh batch.
        if (pdfClearTimer.current && !isTerminal) {
          clearTimeout(pdfClearTimer.current);
          pdfClearTimer.current = null;
          pdfSeen.current.clear();
          pdfCompleted.current.clear();
        }

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
          if (pdfClearTimer.current) clearTimeout(pdfClearTimer.current);
          pdfClearTimer.current = setTimeout(() => {
            useJobStore.getState().remove("pdf_parse");
            pdfSeen.current.clear();
            pdfCompleted.current.clear();
            pdfClearTimer.current = null;
          }, COMPLETED_LINGER_MS);
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
