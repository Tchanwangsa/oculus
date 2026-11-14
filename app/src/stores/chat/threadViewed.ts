import { harnessThreadViewed } from "@/lib/harness";
import { useHarnessStore } from "./harnessStore";

/** Threads whose `harness_thread_viewed` is in flight, so a re-render does
 *  not send it twice. */
const marking = new Set<number>();

/** Stamp the thread as seen in Rust and in the store's list. Only
 *  `viewed_at` changes: `updated_at` orders the list, so a look never moves
 *  a thread. */
export async function markThreadViewed(id: number): Promise<void> {
  if (marking.has(id)) return;
  marking.add(id);
  try {
    const at = await harnessThreadViewed(id);
    if (at == null) return;
    useHarnessStore.setState((s) => {
      const i = s.threads.findIndex((t) => t.id === id);
      if (i < 0) return s;
      const threads = [...s.threads];
      threads[i] = { ...threads[i], viewed_at: at };
      return { threads };
    });
  } catch (e) {
    console.error("marking the thread viewed failed", e);
  } finally {
    marking.delete(id);
  }
}
