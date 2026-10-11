import { useCallback, type Dispatch, type SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  getFileByRelativePath,
  getSyncRunSummaries,
  setSubjectSelected,
  type SyncRunSummary,
} from "@/lib/db";
import { triggerSync } from "@/lib/pipeline/syncRunner";
import { useIndexStore } from "@/stores/sync/indexStore";
import { usePipelineStore, statusOf, type PipelineItem } from "@/stores/sync/pipelineStore";
import { useParseStore } from "@/stores/sync/parseStore";
import { parseFile, parseSkip, parseSkipped } from "@/lib/files/courseFiles";
import type { ActivityView } from "./constants";

interface SyncActionsInput {
  authStatus: string;
  connect: () => void;
  selectedIds: Set<number>;
  setSelectedIds: Dispatch<SetStateAction<Set<number>>>;
  setRuns: Dispatch<SetStateAction<SyncRunSummary[]>>;
  setView: (view: ActivityView) => void;
  setLoadingSubjects: Dispatch<SetStateAction<boolean>>;
  setSubjectsError: Dispatch<SetStateAction<string | null>>;
}

/** Every button on the sync page: subjects, sync, cancel, and per-file resume/skip. */
export function useSyncActions({
  authStatus,
  connect,
  selectedIds,
  setSelectedIds,
  setRuns,
  setView,
  setLoadingSubjects,
  setSubjectsError,
}: SyncActionsInput) {
  const handleRefetchSubjects = async () => {
    setLoadingSubjects(true);
    setSubjectsError(null);
    try {
      await invoke("sync_subjects");
    } catch (err) {
      setLoadingSubjects(false);
      setSubjectsError(String(err));
    }
  };

  const handleCancel = async () => {
    try {
      await invoke("cancel_scrape");
    } catch {
      /* ignore */
    }
  };

  const handleSyncClick = async () => {
    // No live session: open the Canvas sign-in window instead of failing.
    if (authStatus !== "connected") {
      setSubjectsError(null);
      connect();
      return;
    }
    if (selectedIds.size === 0) return;

    setSubjectsError(null);
    setView("history");

    try {
      await triggerSync();
      await getSyncRunSummaries().then(setRuns);
    } catch (err) {
      setSubjectsError(String(err));
    }
  };

  /** Resume one file at the stage it stopped. Both calls are idempotent in
   *  Rust, so resume = retry; the stage picks which call, since re-parsing a
   *  parsed file would leave the pending embed untouched. A skipped file has
   *  its mark lifted and is parsed. */
  const resumeItem = useCallback(async (it: PipelineItem) => {
    // Not on disk, so neither call has anything to work on; the next sync
    // downloads it again.
    if (it.download === "error") return;
    const { touch } = usePipelineStore.getState();
    const embedding = it.parse === "done";

    if (it.parse === "skipped") {
      // Both stores leave the skip at once, or it would swallow the parse's
      // events (`useBackendEvents`).
      const ev = { relative_path: it.relativePath, subject_id: it.subjectId };
      useParseStore.getState().update({ ...ev, status: "queued" });
      touch(it.relativePath, it.subjectId, { parse: "queued", skippedAt: undefined });
      try {
        await parseSkipped(it.subjectId, it.code, it.relativePath);
      } catch (e) {
        useParseStore.getState().update({ ...ev, status: "error", error: String(e) });
        touch(it.relativePath, it.subjectId, { parse: "error", error: String(e) });
      }
      return;
    }

    // Clear paused/failed immediately so the row reads as moving again.
    touch(it.relativePath, it.subjectId, {
      ...(it.parse === "error" ? { parse: "pending" as const } : {}),
      ...(it.embed === "error" ? { embed: "pending" as const } : {}),
      error: undefined,
      errorKind: undefined,
      errorRetryable: undefined,
      errorLatching: undefined,
    });

    if (embedding) {
      // The one serial embed queue, so a retry never opens a second run
      // against the same rate limit.
      const file = await getFileByRelativePath(it.relativePath).catch(() => null);
      if (!file) {
        touch(it.relativePath, it.subjectId, {
          embed: "error",
          error: "This file is not in the database",
        });
        return;
      }
      useIndexStore.getState().enqueueFile(file);
      return;
    }

    try {
      await parseFile(it.subjectId, it.code, it.relativePath);
    } catch (e) {
      touch(it.relativePath, it.subjectId, { parse: "error", error: String(e) });
    }
  }, []);

  /** Stop one file's parse and keep it unparsed; Rust answers `skipped`. */
  const skipItem = useCallback((it: PipelineItem) => {
    parseSkip(it.subjectId, it.relativePath, true).catch((e) =>
      console.error("parse_skip failed", it.relativePath, e),
    );
  }, []);

  /** Resume every paused row; parses batch behind one another and embeds go
   *  into the one serial queue. */
  const resumeAll = useCallback(() => {
    const all = Object.values(usePipelineStore.getState().items);
    const on = usePipelineStore.getState().embedStage;
    for (const it of all) {
      if (statusOf(it, on).phase !== "paused") continue;
      void resumeItem(it);
    }
  }, [resumeItem]);

  const toggleSubject = (id: number) => {
    const nowSelected = !selectedIds.has(id);
    setSelectedIds((prev) => {
      const next = new Set(prev);
      nowSelected ? next.add(id) : next.delete(id);
      return next;
    });
    setSubjectSelected(id, nowSelected).catch((e) =>
      console.error("persist subject selection failed", e),
    );
  };

  return {
    handleRefetchSubjects,
    handleCancel,
    handleSyncClick,
    resumeItem,
    skipItem,
    resumeAll,
    toggleSubject,
  };
}
