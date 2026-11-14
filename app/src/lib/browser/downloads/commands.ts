import { invoke } from "@tauri-apps/api/core";
import type { Dest, DownloadsSnapshot } from "./types";

/** The download commands (`shell/browser/downloads/commands.rs`). Every
 *  result that changes the list arrives on `browser-downloads`. */
export const downloads = {
  /** The whole list, for a mount that missed earlier events. */
  list: () => invoke<DownloadsSnapshot>("browser_downloads"),
  /** Rejects for a row past choosing, an empty subject or a relative path. */
  choose: (id: number, dest: Dest) =>
    invoke<void>("browser_download_choose", { id, dest }),
  /** The native save panel in Downloads; null when cancelled. */
  savePanel: (id: number) =>
    invoke<string | null>("browser_download_save_panel", { id }),
  reveal: (id: number) => invoke<void>("browser_download_reveal", { id }),
  open: (id: number) => invoke<void>("browser_download_open", { id }),
  /** Only a saved, discarded or failed row. */
  clear: (id: number) => invoke<void>("browser_download_clear", { id }),
  /** A failed row: fetched again in a new tab; it returns as a new row. */
  retry: (id: number) => invoke<number>("browser_download_retry", { id }),
};
