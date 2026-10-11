import { invoke } from "@tauri-apps/api/core";
import { getSubjects, getSyncOptions, startSyncRun } from "@/lib/db";
import { useSyncStore } from "@/stores/sync/syncStore";

/** Window event raised once a `scrape-file`'s row is written, so a page that
 *  reloads on it reads the new row rather than racing the upsert. */
export const FILE_SCRAPED_EVENT = "oculus:file-scraped";
export type FileScraped = { subject_id: number; canvas_id: number | null };

/** Syncs the selected subjects; returns the run id its file ledger is keyed
 *  under. */
export async function triggerSync(): Promise<number> {
  if (useSyncStore.getState().scraping) throw new Error("A sync is already running");
  const sel = (await getSubjects())
    .filter((s) => s.selected)
    .map((s) => ({ id: s.id, code: s.code }));
  if (sel.length === 0) throw new Error("No subjects selected");

  const options = await getSyncOptions();
  const runId = await startSyncRun(sel.map((s) => s.code));
  useSyncStore.getState().begin(runId, sel);
  try {
    await invoke("scrape_content", { subjects: sel, options });
  } catch (err) {
    useSyncStore.getState().fail(String(err));
    throw err;
  }
  return runId;
}
