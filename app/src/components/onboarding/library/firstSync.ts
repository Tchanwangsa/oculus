import { create } from "zustand";
import { triggerSync } from "@/lib/pipeline/syncRunner";
import { useSyncStore } from "@/stores/sync/syncStore";

interface FirstSyncState {
  /** True once setup has asked for a sync, whether or not it started. */
  started: boolean;
  /** `useSyncStore.completedAt` when it was asked for: a run that ends bumps
   *  it, so `SyncProgressLine` can tell this run's end from an older one's. */
  baseline: number;
  /** Why it didn't start; `triggerSync`'s sentence. */
  error: string | null;
}

/** The sync setup starts on leaving the Library step, which the Agent step
 *  follows with `SyncProgressLine`. Session-scoped: a reload forgets it. */
export const useFirstSync = create<FirstSyncState>(() => ({
  started: false,
  baseline: 0,
  error: null,
}));

/**
 * Starts the first sync of the subjects selected now. `scrape_content`
 * returns once the run is under way, so this resolves in moments. A sync
 * already running (Back, then Next again) is left alone; any other refusal
 * is kept for `SyncProgressLine`, since the step moves on either way.
 */
export async function startFirstSync(): Promise<void> {
  const sync = useSyncStore.getState();
  if (sync.scraping) return;
  useFirstSync.setState({ started: true, baseline: sync.completedAt, error: null });
  try {
    await triggerSync();
  } catch (cause) {
    const message = cause instanceof Error ? cause.message : String(cause);
    if (message.includes("already running")) return;
    useFirstSync.setState({ error: message });
  }
}
