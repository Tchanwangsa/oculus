import { COURSES_DIR } from "@/lib/files/libraryLayout";
import type { Subject } from "@/lib/db";

/**
 * Path arithmetic the browser needs, none of it naming a library folder: a
 * path's subject, its parent, a free name for a new entry, and the part of a
 * name a rename selects.
 */

/** `courses/<code>`, the subject folder `rel` is in; null outside one. */
export function subjectRelOf(rel: string): string | null {
  const [top, folder] = rel.split("/");
  return top === COURSES_DIR && folder ? `${COURSES_DIR}/${folder}` : null;
}

/** The subject folder's own name (`COMP30026_2026_SM2`), or null. */
export function subjectFolderOf(rel: string): string | null {
  return subjectRelOf(rel)?.split("/")[1] ?? null;
}

export function parentRel(rel: string): string {
  const slash = rel.lastIndexOf("/");
  return slash < 0 ? "" : rel.slice(0, slash);
}

export function baseName(rel: string): string {
  return rel.slice(rel.lastIndexOf("/") + 1);
}

/** True when `rel` is `dir` or below it. */
export function isWithin(rel: string, dir: string): boolean {
  return rel === dir || rel.startsWith(`${dir}/`);
}

/** A code as Rust compares them (`fold_code` over `safe_dir`): the folder a
 *  subject's code was written to, upper-case, with space, `-` and `_` alike. */
export function foldCode(code: string): string {
  return code
    .trim()
    .replace(/[^\p{L}\p{N}_-]/gu, "_")
    .replace(/[\s-]/g, "_")
    .toUpperCase();
}

/** The subject whose folder is `folder`. */
export function subjectForFolder(folder: string, subjects: readonly Subject[]): Subject | undefined {
  const want = foldCode(folder);
  return subjects.find((s) => foldCode(s.code) === want);
}

/** `base`, else `base 2`, `base 3`… — the first not in `taken`, ignoring
 *  case as the disk does. */
export function freeName(base: string, taken: Iterable<string>): string {
  const used = new Set([...taken].map((n) => n.toLowerCase()));
  if (!used.has(base.toLowerCase())) return base;
  for (let n = 2; ; n += 1) {
    const name = `${base} ${n}`;
    if (!used.has(name.toLowerCase())) return name;
  }
}

/** The part of a file name a rename selects at first: all of it but the
 *  extension (`notes.final.pdf` → `notes.final`); a folder's whole name. */
export function stemLength(name: string, folder: boolean): number {
  if (folder) return name.length;
  const dot = name.lastIndexOf(".");
  return dot > 0 ? dot : name.length;
}

/** Why a typed name can't be used, or null. Rust has the last word
 *  (`invalid_name`); this catches what is plain before asking it. */
export function nameProblem(name: string): string | null {
  const trimmed = name.trim();
  if (!trimmed) return "A name can't be empty";
  if (trimmed.startsWith(".")) return "A name can't start with a dot";
  if (/[/\\:]/.test(trimmed)) return "A name can't hold / \\ or :";
  return null;
}
