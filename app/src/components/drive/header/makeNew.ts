import type { Subject } from "@/lib/db";
import { importInto, newFolderIn, reportDriveProblem } from "@/lib/drive/actions";
import { driveErrorText } from "@/lib/drive/commands";
import { announceDriveChange } from "@/lib/drive/listing";
import { createCanvas } from "@/lib/canvas/files";
import { filePagePath } from "@/lib/files/openFile";
import { pickUploads } from "@/lib/files/uploads";
import { createDocument } from "@/lib/notes/documents";
import { navigateActive } from "@/lib/shell/tabRouters";
import type { NewKind } from "./NewMenu";

/**
 * One pick from the New menu, made in `folder`: a folder (then renamed in
 * place), a note or canvas (then opened), a notebook (its dialog asks
 * first), or files and folders chosen from the disk.
 */
export async function makeNew(kind: NewKind, {
  folder,
  subject,
  subjects,
  taken,
  renameFolder,
  openNotebook,
}: {
  folder: string;
  subject: Subject;
  subjects: readonly Subject[];
  /** Names already in the folder, for the new folder's. */
  taken: Iterable<string>;
  renameFolder: (rel: string) => void;
  openNotebook: () => void;
}): Promise<void> {
  try {
    switch (kind) {
      case "folder": {
        const rel = await newFolderIn(folder, taken, subjects);
        if (rel) renameFolder(rel);
        return;
      }
      case "note": {
        const row = await createDocument(subject, "Untitled", folder);
        announceDriveChange(subject.id);
        navigateActive(filePagePath(subject.id, row.relative_path));
        return;
      }
      case "canvas": {
        const row = await createCanvas(subject, "Untitled", undefined, folder);
        announceDriveChange(subject.id);
        navigateActive(filePagePath(subject.id, row.relative_path));
        return;
      }
      case "notebook":
        openNotebook();
        return;
      case "files":
      case "folders": {
        const paths = await pickUploads(kind === "folders");
        if (paths.length) await importInto(paths, folder, subjects);
        return;
      }
    }
  } catch (error) {
    reportDriveProblem(driveErrorText(error));
  }
}
