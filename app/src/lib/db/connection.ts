import Database from "@tauri-apps/plugin-sql";

let _db: Promise<Database> | null = null;

export function getDb(): Promise<Database> {
  // Share initialization across the shell, restored panes and StrictMode mounts.
  // A failed load leaves the next call free to retry, except a failed
  // migration: the plugin hands over its migrations to the first load only,
  // so a retry would open the database without them and hide the error.
  return _db ??= Database.load("sqlite:oculus.db").catch((error) => {
    if (!/migration/i.test(String(error))) _db = null;
    throw error;
  });
}
