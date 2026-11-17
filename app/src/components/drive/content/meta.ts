import { displayName, fmtDay, fmtRelativeDay, fmtSize } from "@/lib/format/format";
import type { DriveItem } from "@/lib/drive/items";
import { isWithin, parentRel } from "@/lib/drive/paths";
import type { DriveView } from "../DriveContext";
import { isLockedFolder } from "./ItemIcon";

export const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? "" : "s"}`;

/** A recording's day, from the lecture's stored date. */
export function recordingDay(date: string): string {
  const ms = Date.parse(date);
  return Number.isNaN(ms) ? "" : fmtDay(new Date(ms));
}

/** The folder an entry sits in, below the browser's root: what the flat
 *  list shows under a name. */
export function folderOf(rel: string, root: string): string {
  const parent = parentRel(rel);
  if (!isWithin(parent, root) || parent === root) return "";
  return root ? parent.slice(root.length + 1) : parent;
}

/** The line under a card's name. */
export function entryMeta(item: DriveItem, view: DriveView): string {
  if (item.type === "recording") return recordingDay(item.entry.date);
  if (item.type === "folder") {
    if (item.subject) return `${displayName(item.subject.name, item.subject.code)} · ${plural(item.entry.childCount, "item")}`;
    if (isLockedFolder(item, view.writable)) return "Synced by Oculus";
    if (view.flat) return folderOf(item.key, view.root) || plural(item.entry.childCount, "item");
    return item.entry.childCount ? plural(item.entry.childCount, "item") : "Empty";
  }
  if (view.flat) return folderOf(item.key, view.root);
  const day = item.entry.modified != null ? fmtRelativeDay(item.entry.modified) : "";
  return [day, fmtSize(item.entry.size)].filter(Boolean).join(" · ");
}
