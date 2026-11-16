import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { HARNESS_THREADS_CHANGED_EVENT } from "@/lib/harness";
import { useHarnessStore } from "./harnessStore";
import { useCompletedStore } from "./completedStore";

let watchers = 0;
let unlisten: Promise<UnlistenFn> | null = null;

/** Re-reads the open list and the completed folds when Rust completes,
 *  reopens or sweeps threads. */
export function threadsChanged(): Promise<void> {
  return Promise.all([useHarnessStore.getState().loadThreads(), useCompletedStore.getState().refresh()])
    .then(() => {})
    .catch((e) => console.error("thread list refresh failed", e));
}

/** One listener however many thread columns are mounted (a Chat tab each);
 *  the returned function lets go of this one's share. */
export function watchThreadsChanged(): () => void {
  if (watchers++ === 0) unlisten = listen<number[]>(HARNESS_THREADS_CHANGED_EVENT, () => void threadsChanged());
  return () => {
    if (--watchers > 0) return;
    void unlisten?.then((off) => off()).catch(() => {});
    unlisten = null;
  };
}
