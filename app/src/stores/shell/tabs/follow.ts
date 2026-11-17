import type { FollowedRoute } from "@/lib/shell/pathFollow";
import type { ClosedTab } from "./persist";

/** Where a route goes after library paths moved (`followRoute` bound to one
 *  change). */
export type RouteFollower = (route: string) => FollowedRoute;

/** The reopen stack after a change, as open tabs follow it: a main path
 *  moves or falls back, a side path moves or is dropped. The same array when
 *  nothing changed. */
export function followClosed(closed: ClosedTab[], follow: RouteFollower): ClosedTab[] {
  let changed = false;
  const next = closed.map((entry) => {
    if (entry.path == null) return entry;
    const main = follow(entry.path);
    const path = main.kind === "moved" ? main.path : main.kind === "removed" ? main.fallback : entry.path;
    const side = entry.side?.flatMap((p) => {
      const item = follow(p);
      return item.kind === "moved" ? [item.path] : item.kind === "removed" ? [] : [p];
    }) ?? null;
    const sideChanged = entry.side != null && (side!.length !== entry.side.length || side!.some((p, i) => p !== entry.side![i]));
    if (path === entry.path && !sideChanged) return entry;
    changed = true;
    return { ...entry, path, side: side && side.length > 0 ? side : null };
  });
  return changed ? next : closed;
}
