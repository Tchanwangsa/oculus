import Database from "@tauri-apps/plugin-sql";

let _db: Promise<Database> | null = null;

export function getDb(): Promise<Database> {
  // Share initialization across the shell, restored panes and StrictMode mounts.
  // A failed load must leave the next call free to retry.
  return _db ??= Database.load("sqlite:oculus.db").catch((error) => {
    _db = null;
    throw error;
  });
}
