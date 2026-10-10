/**
 * The in-app browser's history and site icons. **One row per URL, not per
 * visit**: `visits` and `last_visit` are all the address bar and the by-day
 * view need. Written here from the snapshot the frontend mirrors of
 * `app/src-tauri/src/shell/browser/`, so ranking runs with no IPC.
 */

export * from "./history";
export * from "./favicons";
