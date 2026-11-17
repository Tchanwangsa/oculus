import { COURSE_DIR, FILES_DIR, MEDIA_DIR } from "@/lib/files/libraryLayout";

/**
 * Paths of sub-tabs the Files tab no longer has
 * (`/subjects/<id>/files/downloads` and kin), which saved tabs, the closed
 * stack and Recent may carry. Each names the folder holding those files,
 * best first: Course ▸ Files for Downloads, Course ▸ Media for Media (then Course,
 * for a subject with no video), the subject's own files for the rest.
 */
const LEGACY_TABS = new Map<string, readonly string[]>([
  ["downloads", [FILES_DIR, COURSE_DIR]],
  ["media", [MEDIA_DIR, COURSE_DIR]],
  ["uploads", [""]],
  ["drive", [""]],
  ["documents", [""]],
  ["canvases", [""]],
  ["notebooks", [""]],
]);

/** Where an old sub-tab's path (`inner`, the Files route's path inside the
 *  subject) goes, best first; null for any other path. Exact case: a
 *  student's folder named `Media` is theirs. A folder named `downloads`
 *  is the student's too, which the caller finds first (`resolveLegacyTab`). */
export function legacyTabTargets(inner: string): readonly string[] | null {
  return LEGACY_TABS.get(inner) ?? null;
}

/**
 * The subject-relative folder an old sub-tab path should show: null while
 * `inner` is not one, or when a real folder by that name exists (it is the
 * student's); else the first target that exists, the subject's root last.
 * `exists` asks the disk for a subject-relative folder.
 */
export async function resolveLegacyTab(
  inner: string,
  exists: (subjectRel: string) => Promise<boolean>,
): Promise<string | null> {
  const targets = legacyTabTargets(inner);
  if (!targets || (await exists(inner))) return null;
  for (const target of targets) {
    if (target === "" || (await exists(target))) return target;
  }
  return "";
}
