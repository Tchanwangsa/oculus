/**
 * Top-level paths with no route of their own that a tab, side panel item,
 * closed tab or Recent row kept in localStorage can still hold. Restoring one
 * drops it: the router's catch-all would only open it as a second Home.
 */
const GONE = ["/music"];

export function isGoneRoute(path: string): boolean {
  const pathname = path.split(/[?#]/)[0];
  return GONE.some((p) => pathname === p || pathname.startsWith(`${p}/`));
}
