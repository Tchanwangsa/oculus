import type { DbFile, Subject } from "@/lib/db";
import { fileTitle } from "@/lib/files/openFile";
import { displayCode } from "@/lib/format/format";
import type { DriveZone, FileEntry, FolderEntry, RecordingEntry } from "./listing";
import { subjectForFolder } from "./paths";

/** One entry on screen. `key` is its path, unique in a listing; the order of
 *  a list of these is the order on screen (selection ranges follow it). */
export type DriveItem =
  | { type: "folder"; key: string; name: string; entry: FolderEntry; subject?: Subject }
  | { type: "recording"; key: string; name: string; entry: RecordingEntry }
  | { type: "file"; key: string; name: string; entry: FileEntry; row?: DbFile };

/** `atRoot` folders are subject folders, named by their subject. */
export function folderItem(entry: FolderEntry, subjects: readonly Subject[], atRoot: boolean): DriveItem {
  const subject = atRoot ? subjectForFolder(entry.name, subjects) : undefined;
  return {
    type: "folder",
    key: entry.relativePath,
    name: subject ? displayCode(subject.code) : entry.name,
    entry,
    subject,
  };
}

export function recordingItem(entry: RecordingEntry): DriveItem {
  return { type: "recording", key: entry.relativePath, name: entry.title, entry };
}

/** A file is named as its row names it (a note without `.md`, a synced
 *  page's slug humanised), else as on disk. */
export function fileItem(entry: FileEntry, rows: ReadonlyMap<string, DbFile>): DriveItem {
  const row = rows.get(entry.relativePath);
  return { type: "file", key: entry.relativePath, name: row ? fileTitle(row) : entry.name, entry, row };
}

export function itemZone(item: DriveItem): DriveZone {
  return item.type === "recording" ? "synced" : item.entry.zone;
}

/** The student may rename, move and delete it. */
export function itemEditable(item: DriveItem): boolean {
  return item.type !== "recording" && item.entry.editable;
}

/** The name on disk, which a rename starts from. */
export function diskName(item: DriveItem): string {
  return item.type === "recording" ? item.entry.title : item.entry.name;
}
