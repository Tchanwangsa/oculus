import { setEmbedStatus } from "@/lib/db";
import { reportEmbedPages } from "@/stores/sync/indexStore";
import { useTauriEvent } from "@/hooks/backend/useEvents";
import {
  EMBED_STATUSES,
  NO_WAIT,
  clearErrorUnless,
  parseIfUnseen,
  pipeline,
  type EmbedJob,
} from "./pipelineEvents";

/** Page embedding stage events. */
export function useEmbedEvents() {
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
        touch(path, ev.subject_id, { ...parseIfUnseen(path), embed: "queued", ...NO_WAIT });
        break;
      case "running":
        touch(path, ev.subject_id, {
          ...parseIfUnseen(path),
          embed: "active",
          embedPagesDone: ev.pages_done ?? 0,
          embedTotalPages: ev.total_pages ?? 0,
          // Absent fields clear a wait the previous event set.
          embedWaitingUntil: ev.waiting_until_ms,
          embedWaitingReason: ev.waiting_reason,
        });
        reportEmbedPages(path, ev.pages_done ?? 0, ev.total_pages ?? 0);
        break;
      case "done":
        touch(path, ev.subject_id, {
          ...parseIfUnseen(path),
          embed: "done",
          embedPagesDone: ev.pages_done ?? 0,
          embedTotalPages: ev.total_pages ?? 0,
          embeddedAt: Date.now(),
          ...NO_WAIT,
          ...clearErrorUnless(path, "parse"),
        });
        break;
      case "error":
        touch(path, ev.subject_id, {
          ...parseIfUnseen(path),
          embed: "error",
          error: ev.error ?? "Embedding failed",
          errorKind: ev.kind,
          errorRetryable: ev.retryable,
          errorLatching: ev.latching,
          ...NO_WAIT,
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
