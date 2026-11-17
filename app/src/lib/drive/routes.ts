import { COURSES_DIR } from "@/lib/files/libraryLayout";

/**
 * The two routes that show a library folder, and their parsers. Drive's
 * splat is the path under `courses/` (`/drive/<code>/Course/Files`); a
 * subject's Files tab's splat is the path inside that subject
 * (`/subjects/<id>/files/Course/Files`). Each segment is URL-encoded on its
 * own, so a folder name may hold spaces, `#` or `?`. Drive's Recent list is
 * `/drive/recent`, a name no course code takes.
 */

export const DRIVE_HREF = "/drive";
export const DRIVE_RECENT_HREF = "/drive/recent";

const encode = (path: string): string =>
  path.split("/").filter(Boolean).map(encodeURIComponent).join("/");

function decode(splat: string): string {
  return splat
    .split("/")
    .filter(Boolean)
    .map((segment) => {
      try {
        return decodeURIComponent(segment);
      } catch {
        return segment;
      }
    })
    .join("/");
}

/** Drive's route for a data-dir-relative folder: `""` (or `courses`) is the
 *  Drive root. */
export function driveFolderHref(rel: string): string {
  const under = rel === COURSES_DIR ? "" : rel.startsWith(`${COURSES_DIR}/`) ? rel.slice(COURSES_DIR.length + 1) : "";
  const splat = encode(under);
  return splat ? `${DRIVE_HREF}/${splat}` : DRIVE_HREF;
}

/** A subject's Files tab on a folder inside the subject (`""` is its root). */
export function filesFolderHref(subjectId: number, subjectRel: string): string {
  const splat = encode(subjectRel);
  return splat ? `/subjects/${subjectId}/files/${splat}` : `/subjects/${subjectId}/files`;
}

/** The data-dir-relative folder a Drive route shows (`""` at the root), or
 *  null for any other route. Takes a pathname or a full path with a query. */
export function parseDriveRoute(pathname: string): string | null {
  const m = /^\/drive(?:\/([^?#]*))?(?:[?#].*)?$/.exec(pathname);
  if (!m || m[1] === "recent") return null;
  const under = decode(m[1] ?? "");
  return under ? `${COURSES_DIR}/${under}` : "";
}

/** The subject and the folder inside it (`""` at its root) a Files route
 *  shows, or null for any other route. */
export function parseFilesRoute(pathname: string): { subjectId: number; inner: string } | null {
  const m = /^\/subjects\/(\d+)\/files(?:\/([^?#]*))?(?:[?#].*)?$/.exec(pathname);
  if (!m) return null;
  return { subjectId: Number(m[1]), inner: decode(m[2] ?? "") };
}
