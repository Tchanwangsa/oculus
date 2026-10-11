import { useEffect, useRef, type Dispatch, type MutableRefObject, type SetStateAction } from "react";
import { addLog, upsertSubjects, type CanvasCourseRaw } from "@/lib/db";
import { useTauriEvent } from "@/hooks/backend/useEvents";
import { useSyncStore } from "@/stores/sync/syncStore";

interface SyncEventsInput {
  loadFromDb: () => Promise<void>;
  requestSeed: () => void;
  activeRef: MutableRefObject<boolean>;
  completedAt: number;
  syncError: string | null;
  setLoadingSubjects: Dispatch<SetStateAction<boolean>>;
  setSubjectsError: Dispatch<SetStateAction<string | null>>;
}

/** Canvas auth and subject events, and what a finished sync run refreshes. */
export function useSyncEvents({
  loadFromDb,
  requestSeed,
  activeRef,
  completedAt,
  syncError,
  setLoadingSubjects,
  setSubjectsError,
}: SyncEventsInput) {
  useTauriEvent("canvas-auth-expired", () => {
    if (useSyncStore.getState().scraping) {
      useSyncStore.getState().reset();
      setSubjectsError(
        "Canvas session expired during sync. Reconnect and try again.",
      );
    }
  });

  useTauriEvent<CanvasCourseRaw[]>("subjects-loaded", async (e) => {
    setLoadingSubjects(false);
    setSubjectsError(null);
    try {
      await upsertSubjects(e.payload);
      await addLog(`Fetched ${e.payload.length} subjects from Canvas`);
      await loadFromDb();
    } catch (err) {
      console.error("DB upsert failed:", err);
    }
  });
  useTauriEvent<string>("subjects-error", (e) => {
    setSubjectsError(e.payload);
    setLoadingSubjects(false);
  });

  // Refresh subjects and the run table when a sync run finishes.
  useEffect(() => {
    if (completedAt === 0) return;
    if (syncError) {
      setSubjectsError(`Sync error: ${syncError}`);
    }
    loadFromDb();
  }, [completedAt, syncError, loadFromDb]);

  // A run that ends (or fails partway) has written file rows. Only a run
  // ending after mount: activation already seeds, and the store's count
  // outlives the page.
  const seenCompletion = useRef(completedAt);
  useEffect(() => {
    if (completedAt === seenCompletion.current) return;
    seenCompletion.current = completedAt;
    if (activeRef.current) requestSeed();
  }, [completedAt, requestSeed]);
}
