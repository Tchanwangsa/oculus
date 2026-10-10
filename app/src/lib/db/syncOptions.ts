import { getSetting, setSetting } from "./settings";

/** What a sync fetches. Mirrors `SyncOptions` in `app/src-tauri/src/sync/mod.rs`. */
export interface SyncOptions {
  announcements: boolean;
  /** Assignments and quizzes. */
  assignments: boolean;
  /** Module pages and files. */
  modules: boolean;
  ed: boolean;
  /** Echo360 lecture list only, refreshed by the frontend after the scrape;
   *  never downloads videos. */
  lectures: boolean;
  /** Class times and due dates; also a frontend post-scrape refresh. */
  calendar: boolean;
}

export const DEFAULT_SYNC_OPTIONS: SyncOptions = {
  announcements: true,
  assignments: true,
  modules: true,
  ed: true,
  lectures: true,
  calendar: true,
};

const SYNC_OPTIONS_KEY = "sync-options";

export async function getSyncOptions(): Promise<SyncOptions> {
  const raw = await getSetting(SYNC_OPTIONS_KEY);
  if (!raw) return { ...DEFAULT_SYNC_OPTIONS };
  try {
    // Merge over defaults so options added later default on for old settings.
    return { ...DEFAULT_SYNC_OPTIONS, ...JSON.parse(raw) };
  } catch {
    return { ...DEFAULT_SYNC_OPTIONS };
  }
}

export async function setSyncOptions(options: SyncOptions): Promise<void> {
  await setSetting(SYNC_OPTIONS_KEY, JSON.stringify(options));
}
