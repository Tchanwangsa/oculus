import { invoke } from "@tauri-apps/api/core";
import {
  getSubjects, getSyncOptions, startSyncRun,
  type SyncOrigin,
} from "@/lib/db";
import { useSyncStore } from "@/stores/syncStore";

/**
 * Kick off a sync of the currently selected subjects.
 *
 * The Sync page's button is the only caller since automations were removed, so
 * every run is `manual` today; the `origin` parameter stays because `sync_runs`
 * already records it and rows written by the old scheduler still say
 * `scheduled`.
 *
 * Returns the run id — the key the run's file ledger is written under.
 */
export async function triggerSync(origin: SyncOrigin): Promise<number> {
  if (useSyncStore.getState().scraping) throw new Error("A sync is already running");
  const sel = (await getSubjects())
    .filter((s) => s.selected)
    .map((s) => ({ id: s.id, code: s.code }));
  if (sel.length === 0) throw new Error("No subjects selected");

  const options = await getSyncOptions();
  const runId = await startSyncRun(sel.map((s) => s.code), origin);
  useSyncStore.getState().begin(runId, sel);
  try {
    await invoke("scrape_content", { subjects: sel, options });
  } catch (err) {
    useSyncStore.getState().fail(String(err));
    throw err;
  }
  return runId;
}
