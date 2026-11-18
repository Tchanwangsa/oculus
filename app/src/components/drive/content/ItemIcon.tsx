import { FolderSimple, LockSimple, MonitorPlay } from "@phosphor-icons/react";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { categoryIconFor, fileIconFor } from "@/lib/files/fileTypes";
import { kindOf, type DriveKind } from "@/lib/drive/filters";
import type { DriveItem } from "@/lib/drive/items";
import { cn } from "@/lib/utils";

/** Each kind's glyph colour: the `reference-*` tokens file rows use. */
const KIND_TONE: Partial<Record<DriveKind, string>> = {
  pdf: "text-reference-pdf",
  document: "text-reference-document",
  slides: "text-reference-document",
  sheet: "text-reference-lesson",
  note: "text-reference-page",
  whiteboard: "text-reference-page",
  notebook: "text-reference-page",
  video: "text-reference-lecture",
};

export function kindTone(item: DriveItem): string {
  if (item.type === "recording") return "text-reference-lecture";
  if (item.type === "folder") return "text-muted-foreground";
  return fileKindTone(item.entry.name, item.row?.category);
}

/** A file's glyph colour, from its name and its row's category. */
export function fileKindTone(name: string, category?: string | null): string {
  return KIND_TONE[kindOf(name, category)] ?? "text-muted-foreground";
}

/** True for a folder at the edge of the synced files: one the student can't
 *  add to, inside one they can (where it is the odd one out). */
export function isLockedFolder(item: DriveItem, parentWritable: boolean): boolean {
  return item.type === "folder" && parentWritable && !item.entry.writable;
}

/** An entry's glyph: a subject's own icon at the Drive root, a lock on a
 *  `locked` folder, a recording's screen, a file's kind. */
export function ItemIcon({ item, size = 15, locked = false, className }: {
  item: DriveItem;
  size?: number;
  locked?: boolean;
  className?: string;
}) {
  const tone = cn("shrink-0", kindTone(item), className);
  if (item.type === "folder") {
    if (item.subject) return <SubjectIcon code={item.subject.code} size={size} className={className} />;
    const Icon = locked ? LockSimple : FolderSimple;
    return <Icon size={size} weight={locked ? "regular" : "fill"} className={tone} aria-hidden />;
  }
  if (item.type === "recording") return <MonitorPlay size={size} className={tone} aria-hidden />;
  const Icon = item.row ? categoryIconFor(item.row) : fileIconFor(item.entry.name);
  return <Icon size={size} weight={kindOf(item.entry.name) === "pdf" ? "fill" : "regular"} className={tone} aria-hidden />;
}
