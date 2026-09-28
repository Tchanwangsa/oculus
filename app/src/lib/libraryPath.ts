/** Resolve agent tool paths and markdown links to a library-relative path.
 * Windows separators, encoded spaces and line citations are normalized. An
 * absolute path is recognized only under the Oculus application data folder;
 * traversal and unrelated absolute paths remain ordinary text. */
export function libraryPath(raw: string | null | undefined): string | null {
  if (!raw || /[\r\n\0]/.test(raw)) return null;
  let path = raw.trim();
  try { path = decodeURI(path); } catch { /* A literal percent is a filename. */ }
  path = path.replace(/\\/g, "/").replace(/:\d+(?:-\d+)?$/, "");
  if (/^(?:[a-z]:\/|\/)/i.test(path)) {
    const match = /(?:^|\/)com\.tchan\.oculus\/(courses\/.+)$/i.exec(path);
    if (!match) return null;
    path = match[1];
  } else {
    path = path.replace(/^\.\.?\//, "");
  }
  if (!path.startsWith("courses/")) return null;
  const parts = path.split("/");
  if (parts.some((part) => !part || part === "." || part === ".." || /[\r\n\0]/.test(part))) return null;
  return path;
}
