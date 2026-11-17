import type { Subject } from "@/lib/db";
import { addUploads } from "@/lib/files/uploads";
import { parseFile } from "@/lib/files/courseFiles";
import { isPipelineFile } from "@/lib/files/fileTypes";
import { copyToOwn, createFolder, deleteEntries, driveErrorText, moveEntries, renameEntry } from "./commands";
import { announceDriveChange } from "./listing";
import { baseName, freeName, subjectFolderOf, subjectForFolder } from "./paths";

/**
 * What Drive does to the library, each followed by `announceDriveChange` so
 * every listing, tree and file list of the subject reloads. A failure is
 * reported through {@link reportDriveProblem}; the browser shows it in a line
 * above its entries (no toasts).
 */

export const DRIVE_PROBLEM_EVENT = "oculus:drive-problem";

export function reportDriveProblem(text: string): void {
  window.dispatchEvent(new CustomEvent(DRIVE_PROBLEM_EVENT, { detail: text }));
}

/** The subject `rel` is in, from the subject list. */
export const subjectOf = (rel: string, subjects: readonly Subject[]): Subject | undefined => {
  const folder = subjectFolderOf(rel);
  return folder ? subjectForFolder(folder, subjects) : undefined;
};

async function attempt<T>(rel: string, subjects: readonly Subject[], run: () => Promise<T>): Promise<T | null> {
  try {
    return await run();
  } catch (error) {
    reportDriveProblem(driveErrorText(error));
    return null;
  } finally {
    announceDriveChange(subjectOf(rel, subjects)?.id);
  }
}

/** Finder files or folders copied into `targetDir`. */
export async function importInto(paths: string[], targetDir: string, subjects: readonly Subject[]): Promise<void> {
  const subject = subjectOf(targetDir, subjects);
  if (!subject) return reportDriveProblem("Files can only be added inside a subject");
  await attempt(targetDir, subjects, async () => {
    const outcomes = await addUploads(subject, paths, targetDir);
    const failed = outcomes.filter((o) => o.error).map((o) => `${o.source}: ${o.error}`);
    if (failed.length) reportDriveProblem(failed.join("\n"));
  });
}

/** Own entries moved and synced ones copied into `targetDir`; each copy is
 *  parsed as its own file. */
export async function dropInto(
  plan: { move: string[]; copy: string[] },
  targetDir: string,
  subjects: readonly Subject[],
): Promise<void> {
  await attempt(targetDir, subjects, async () => {
    if (plan.move.length) await moveEntries(plan.move, targetDir);
    if (plan.copy.length) await copyInto(plan.copy, targetDir, subjects);
  });
}

async function copyInto(rels: string[], targetDir: string, subjects: readonly Subject[]): Promise<void> {
  const copies = await copyToOwn(rels, targetDir);
  const subject = subjectOf(targetDir, subjects);
  if (!subject) return;
  for (const rel of copies) {
    if (isPipelineFile(baseName(rel))) {
      parseFile(subject.id, subject.code, rel).catch((e) => console.warn(`[drive] parse ${rel}: ${e}`));
    }
  }
}

/** Synced entries copied into the student's `targetDir`. */
export async function copyToFolder(rels: string[], targetDir: string, subjects: readonly Subject[]): Promise<void> {
  await attempt(targetDir, subjects, () => copyInto(rels, targetDir, subjects));
}

export async function moveToFolder(rels: string[], targetDir: string, subjects: readonly Subject[]): Promise<void> {
  await attempt(targetDir, subjects, () => moveEntries(rels, targetDir));
}

/** The entry's new path, or null when the rename failed. */
export function renameTo(rel: string, name: string, subjects: readonly Subject[]): Promise<string | null> {
  return attempt(rel, subjects, () => renameEntry(rel, name.trim()));
}

export async function deleteAll(rels: string[], subjects: readonly Subject[]): Promise<boolean> {
  return (await attempt(rels[0] ?? "", subjects, async () => { await deleteEntries(rels); return true; })) ?? false;
}

/** A new folder in `dir`, named clear of `taken`; its path, or null. */
export function newFolderIn(dir: string, taken: Iterable<string>, subjects: readonly Subject[]): Promise<string | null> {
  return attempt(dir, subjects, () => createFolder(`${dir}/${freeName("Untitled folder", taken)}`));
}
