/**
 * Keeps path-keyed state on the library's files when Drive moves, renames or
 * deletes them (`drive-changed`): open tabs and side items, the reopen
 * stack, the new-tab page's Recent trail, and the parse, pipeline and embed
 * queues. Also, once a session, follows saved tabs and Recent rows written
 * before the library moved into the Drive tree through `path_moves`. The
 * rules are `pathFollow.ts`'s; docs/frontend.md has them.
 */
import { getFileByRelativePath, getSubjects } from "@/lib/db";
import { subjectForFolder } from "@/lib/drive/paths";
import { courseFileHasContent } from "@/lib/files/courseFiles";
import { COURSES_DIR } from "@/lib/files/libraryLayout";
import { forgetPathMoves, resolveMoved } from "@/lib/files/pathMoves";
import { followPath, followRoute, type PathChange, type SubjectIdOf } from "@/lib/shell/pathFollow";
import { useRecentTabsStore } from "@/stores/shell/recentTabsStore";
import { panesOf, useTabStore } from "@/stores/shell/tabStore";
import { followIndexQueue } from "@/stores/sync/indexStore";
import { useParseStore } from "@/stores/sync/parseStore";
import { usePipelineStore } from "@/stores/sync/pipelineStore";

async function subjectIdOf(): Promise<SubjectIdOf> {
  const subjects = await getSubjects().catch((e) => {
    console.warn(`[oculus] following moved paths without subjects: ${e}`);
    return [];
  });
  return (folder) => subjectForFolder(folder, subjects)?.id;
}

/** Tabs, the reopen stack and Recent follow `change`. */
function followRoutes(change: PathChange, idOf: SubjectIdOf): void {
  const follow = (route: string) => followRoute(route, change, idOf);
  useTabStore.getState().followRoutes(follow);
  useRecentTabsStore.getState().follow(follow);
}

/** Everything path-keyed follows one `drive-changed`, and a move may have
 *  added `path_moves` rules (a renamed `.canvas`). */
export async function followLibraryChange(change: PathChange): Promise<void> {
  if (change.moved.length) forgetPathMoves();
  if (!change.moved.length && !change.removed.length) return;
  const idOf = await subjectIdOf();
  followRoutes(change, idOf);

  const follow = (path: string) => followPath(path, change);
  const subjectOf = (path: string) => {
    const [top, folder] = path.split("/");
    return top === COURSES_DIR && folder ? idOf(folder) : undefined;
  };
  useParseStore.getState().follow(follow);
  usePipelineStore.getState().follow(follow, subjectOf);
  followIndexQueue(follow, subjectOf);
}

/** The library path a file route names, or null. */
function filePathOf(route: string): string | null {
  const [pathname, search = ""] = route.split("?");
  if (!/^\/subjects\/\d+\/file$/.test(pathname)) return null;
  return new URLSearchParams(search).get("path") || null;
}

/** True when nothing is at `rel`: no row and no file on disk. */
async function missing(rel: string): Promise<boolean> {
  if (await getFileByRelativePath(rel)) return false;
  return !(await courseFileHasContent(rel).catch(() => false));
}

let resolved = false;

/**
 * Once a session, after the first render: a saved tab, side item, closed
 * tab or Recent row on a file that has no row and no file is looked up in
 * `path_moves` (the one-shot library move), and follows it as a Drive move
 * would. A path still found nowhere is left for its page to say so.
 */
export async function followSavedPaths(): Promise<void> {
  if (resolved) return;
  resolved = true;
  const { tabs, closed } = useTabStore.getState();
  const routes = [
    ...tabs.flatMap((t) => panesOf(t).map((p) => p.path)),
    ...closed.flatMap((c) => [c.path ?? "", ...(c.side ?? [])]),
    ...useRecentTabsStore.getState().recents.map((r) => r.path),
  ];
  const rels = [...new Set(routes.map(filePathOf).filter((r): r is string => r != null))];
  const moved: { from: string; to: string }[] = [];
  for (const rel of rels) {
    try {
      if (!(await missing(rel))) continue;
      const to = await resolveMoved(rel);
      if (to && to !== rel) moved.push({ from: rel, to });
    } catch (e) {
      console.warn(`[oculus] could not resolve saved path ${rel}: ${e}`);
    }
  }
  if (moved.length) followRoutes({ moved, removed: [] }, await subjectIdOf());
}
