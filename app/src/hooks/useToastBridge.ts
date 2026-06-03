import { useEffect, useRef } from "react";
import { useToast } from "@/components/ui/toast";
import { useSyncStore } from "@/stores/syncStore";
import { useParseStore } from "@/stores/parseStore";

const SYNC_ID = "sync";
const PARSE_ID = "pdf-parse";

const PHASE_LABEL: Record<string, string> = {
  home: "Overview",
  announcements: "Announcements",
  modules: "Modules",
};

function fileName(p: string) {
  return p.split("/").pop() ?? p;
}

/**
 * Derives toasts from the global stores. Mount once near the app root, inside
 * ToastProvider. Reactive — toasts update/dismiss as store state changes.
 */
export function useToastBridge() {
  const { push, dismiss } = useToast();

  const syncProgress = useSyncStore((s) => s.progress);
  const syncResult = useSyncStore((s) => s.lastResult);
  const syncError = useSyncStore((s) => s.error);
  const jobs = useParseStore((s) => s.jobs);

  // ── Sync toast ────────────────────────────────────────────────────────────
  useEffect(() => {
    if (!syncProgress) return;
    const { done, total, course, phase, label } = syncProgress;
    const phaseLabel = phase ? (PHASE_LABEL[phase] ?? phase) : "";
    const detail = [phaseLabel, label].filter(Boolean).join(" · ");
    push({
      id: SYNC_ID,
      kind: "progress",
      title: course ? `Syncing ${course}` : "Syncing…",
      detail: detail || undefined,
      progress: total ? Math.round((done / total) * 100) : undefined,
    });
  }, [syncProgress, push]);

  const lastResultRef = useRef(syncResult);
  useEffect(() => {
    if (syncResult && syncResult !== lastResultRef.current) {
      lastResultRef.current = syncResult;
      push({
        id: SYNC_ID,
        kind: syncResult.cancelled ? "info" : "success",
        title: syncResult.cancelled ? "Sync cancelled" : "Sync complete",
        detail: syncResult.cancelled
          ? undefined
          : `${syncResult.count} subject${syncResult.count === 1 ? "" : "s"} synced`,
        duration: 3000,
      });
    }
  }, [syncResult, push]);

  useEffect(() => {
    if (syncError) {
      push({ id: SYNC_ID, kind: "error", title: "Sync error", detail: syncError, duration: 5000 });
    }
  }, [syncError, push]);

  // ── PDF parse toast (aggregate) ───────────────────────────────────────────
  const hadJobs = useRef(false);
  useEffect(() => {
    const list = Object.values(jobs);
    if (list.length === 0) {
      if (hadJobs.current) {
        hadJobs.current = false;
        push({ id: PARSE_ID, kind: "success", title: "PDF parsing complete", duration: 2500 });
      }
      return;
    }
    hadJobs.current = true;

    // The file actively being parsed (running takes precedence over queued).
    const running = list.find((j) => j.status === "running");
    const current = running ?? list[0];
    const queuedCount = list.filter((j) => j.status === "queued").length;

    let detail = fileName(current.relative_path);
    let progress: number | undefined;
    if (running && running.total_pages) {
      detail += ` · ${running.pages_done}/${running.total_pages} pages`;
      progress = Math.round((running.pages_done! / running.total_pages) * 100);
    } else if (running) {
      detail += " · starting…";
    } else {
      detail += " · queued";
    }

    push({
      id: PARSE_ID,
      kind: "progress",
      title: list.length > 1 ? `Parsing PDFs · ${queuedCount} queued` : "Parsing PDF",
      detail,
      progress,
    });
  }, [jobs, push]);

  void dismiss;
}
