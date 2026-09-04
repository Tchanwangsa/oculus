import { invoke } from "@tauri-apps/api/core";
import {
  getSubjects, getSyncOptions, startSyncRun,
  type SyncOrigin,
} from "@/lib/db";
import { useSyncStore } from "@/stores/syncStore";

/**
 * Kick off a sync of the currently selected subjects. Shared by the Sync
 * page's button and the scheduler, so scheduled runs go through exactly the
 * same path as manual ones — only the recorded origin differs.
 *
 * Returns the run id: an automation's sync node hands the run's file lists on
 * down the graph, and the ledger those come from is keyed by it.
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
