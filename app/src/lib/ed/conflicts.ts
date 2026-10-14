/**
 * Conflicts between the library copy and Ed's. When both changed a file
 * since the last sync, Rust keeps ours in place and writes Ed's beside it as
 * `<name>.ed-server.<ext>` (`<name>.ed-server` without an extension), then
 * emits `conflict`. The copy on disk is the record, so a conflict survives a
 * reload: the file list alone says which files are in conflict.
 */

/** The `.ed-server` segment, before the file's last extension or at its end. */
const SERVER_COPY = /\.ed-server(\.[^./]*)?$/;

/** The file a server copy stands beside ("src/a.ed-server.py" → "src/a.py"),
 *  or null when `path` is not one. */
export function conflictOriginal(path: string): string | null {
  const slash = path.lastIndexOf("/");
  const dir = path.slice(0, slash + 1);
  const name = path.slice(slash + 1);
  const m = SERVER_COPY.exec(name);
  if (!m || m.index === 0) return null;
  return dir + name.slice(0, m.index) + (m[1] ?? "");
}

export interface WorkspaceListing {
  /** The files to show as tabs: everything but the server copies. */
  files: string[];
  /** File in conflict → its server copy. */
  conflicts: Map<string, string>;
}

/** Splits Rust's file list into the tabs and the conflicts: server copies
 *  found beside listed files, plus those `conflict` events named (`known`).
 *  A copy whose file isn't listed is no conflict, so it stays a tab. */
export function splitListing(
  paths: readonly string[],
  known: ReadonlyMap<string, string> = new Map(),
): WorkspaceListing {
  const listed = new Set(paths);
  const conflicts = new Map<string, string>();
  for (const p of paths) {
    const original = conflictOriginal(p);
    if (original != null && listed.has(original)) conflicts.set(original, p);
  }
  for (const [path, serverPath] of known) if (listed.has(path)) conflicts.set(path, serverPath);
  const copies = new Set(conflicts.values());
  return { files: paths.filter((p) => !copies.has(p)), conflicts };
}
