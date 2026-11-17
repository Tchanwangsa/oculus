/**
 * Where a library path, and a route that names one, went after Drive moved,
 * renamed or deleted entries (`drive-changed`). Pure: the caller passes the
 * change and a way from a subject's folder name to its id, and applies the
 * answer to tabs, Recent and the job queues (`lib/shell/followLibrary.ts`).
 *
 * A folder's entries move or go with it, matched by prefix at a `/`
 * boundary, so `a/b` covers `a/b/c` but not `a/bc`. A file route on a
 * deleted file falls back to the Files tab on the folder that held it; a
 * folder route on a deleted folder moves up to the folder that held it.
 */
import { COURSES_DIR } from "@/lib/files/libraryLayout";
import { filePagePath } from "@/lib/files/labels";
import { driveFolderHref, filesFolderHref, parseDriveRoute, parseFilesRoute } from "@/lib/drive/routes";

export interface PathChange {
  moved: readonly { from: string; to: string }[];
  removed: readonly string[];
}

/** A subject folder's name (`courses/<name>`) to the subject's id. */
export type SubjectIdOf = (folder: string) => number | undefined;

export type FollowedPath =
  | { kind: "same" }
  | { kind: "moved"; path: string }
  | { kind: "removed" };

export type FollowedRoute =
  | { kind: "same" }
  | { kind: "moved"; path: string }
  /** The thing it showed is gone; `fallback` is where its pane goes. */
  | { kind: "removed"; fallback: string };

const SAME = { kind: "same" } as const;

const covers = (prefix: string, path: string) => path === prefix || path.startsWith(`${prefix}/`);

/** Where data-dir-relative `path` is after `change`. */
export function followPath(path: string, change: PathChange): FollowedPath {
  let best: { from: string; to: string } | null = null;
  for (const move of change.moved) {
    if (covers(move.from, path) && (!best || move.from.length > best.from.length)) best = move;
  }
  if (best) return { kind: "moved", path: best.to + path.slice(best.from.length) };
  if (change.removed.some((gone) => covers(gone, path))) return { kind: "removed" };
  return SAME;
}

/** The subject folder (`courses/<name>/…` → `<name>`) and the path inside it. */
function splitSubject(rel: string): { folder: string; inner: string } | null {
  const parts = rel.split("/");
  if (parts[0] !== COURSES_DIR || !parts[1]) return null;
  return { folder: parts[1], inner: parts.slice(2).join("/") };
}

const parentOf = (rel: string) => rel.slice(0, Math.max(0, rel.lastIndexOf("/")));

/** The removed entry `path` sat under: what is left of the tree above it. */
function removedRoot(path: string, change: PathChange): string {
  const hit = change.removed.filter((gone) => covers(gone, path)).sort((a, b) => a.length - b.length)[0];
  return hit ?? path;
}

/** The Files tab route for a data-dir-relative folder, else Drive's. */
function folderRoute(rel: string, fallbackId: number | undefined, idOf: SubjectIdOf): string {
  const split = splitSubject(rel);
  const id = split ? (idOf(split.folder) ?? fallbackId) : undefined;
  return split && id != null ? filesFolderHref(id, split.inner) : driveFolderHref(rel);
}

/** Where a route goes after `change`: a file page follows its file (and its
 *  subject, across a move between subjects), a folder page its folder. */
export function followRoute(route: string, change: PathChange, idOf: SubjectIdOf): FollowedRoute {
  const [pathname, search = ""] = splitQuery(route);

  const file = /^\/subjects\/(\d+)\/file$/.exec(pathname);
  if (file) {
    const params = new URLSearchParams(search);
    const rel = params.get("path");
    if (!rel) return SAME;
    const subjectId = Number(file[1]);
    const next = followPath(rel, change);
    if (next.kind === "same") return SAME;
    if (next.kind === "removed") {
      return { kind: "removed", fallback: folderRoute(parentOf(rel), subjectId, idOf) };
    }
    const from = splitSubject(rel);
    const to = splitSubject(next.path);
    const sameSubject = from && to && from.folder === to.folder;
    const id = sameSubject ? subjectId : ((to && idOf(to.folder)) ?? subjectId);
    params.delete("path");
    const rest = params.toString();
    return { kind: "moved", path: filePagePath(id, next.path) + (rest ? `&${rest}` : "") };
  }

  const drive = parseDriveRoute(pathname);
  if (drive != null) {
    const next = followPath(drive, change);
    if (next.kind === "same") return SAME;
    const rel = next.kind === "moved" ? next.path : parentOf(removedRoot(drive, change));
    return { kind: "moved", path: driveFolderHref(rel) + (search ? `?${search}` : "") };
  }

  const files = parseFilesRoute(pathname);
  if (files) {
    // The change speaks of data-dir paths; find the subject folder it names
    // whose id is this route's.
    const folders = [...change.moved.flatMap((m) => [m.from, m.to]), ...change.removed]
      .map((rel) => splitSubject(rel)?.folder)
      .filter((f): f is string => f != null && idOf(f) === files.subjectId);
    const folder = folders[0];
    if (folder == null) return SAME;
    const rel = files.inner ? `${COURSES_DIR}/${folder}/${files.inner}` : `${COURSES_DIR}/${folder}`;
    const next = followPath(rel, change);
    if (next.kind === "same") return SAME;
    const target = next.kind === "moved" ? next.path : parentOf(removedRoot(rel, change));
    // A subject's own folder is never moved or deleted by Drive; if it were,
    // the Drive root is what is left.
    if (!splitSubject(target)) return { kind: "moved", path: driveFolderHref(target) };
    return { kind: "moved", path: folderRoute(target, files.subjectId, idOf) + (search ? `?${search}` : "") };
  }

  return SAME;
}

function splitQuery(route: string): [string, string] {
  const at = route.indexOf("?");
  return at < 0 ? [route, ""] : [route.slice(0, at), route.slice(at + 1)];
}

/**
 * A path-keyed record after `follow`: a moved key takes its new path (`move`
 * rewrites the value, for one that also carries the path), a removed one
 * goes, and a value `keep` holds stays where it is. The same object when
 * nothing changed.
 */
export function rekeyByPath<T>(
  record: Record<string, T>,
  follow: (path: string) => FollowedPath,
  move: (value: T, path: string) => T = (value) => value,
  keep: (value: T) => boolean = () => false,
): Record<string, T> {
  let changed = false;
  const out: Record<string, T> = {};
  for (const [path, value] of Object.entries(record)) {
    const next = keep(value) ? SAME : follow(path);
    if (next.kind === "same") {
      if (!(path in out)) out[path] = value;
      continue;
    }
    changed = true;
    if (next.kind === "moved") out[next.path] = move(value, next.path);
  }
  return changed ? out : record;
}
