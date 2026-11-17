import { RecencyDot } from "@/components/files/FileRecency";
import { DRAG_SURFACE } from "@/hooks/gestures/usePointerDrag";
import type { DriveItem } from "@/lib/drive/items";
import { fmtRelativeDay, fmtSize } from "@/lib/format/format";
import { cn } from "@/lib/utils";
import { useDriveView } from "../DriveContext";
import { EntryMenu } from "../menu/EntryMenu";
import { EntryName } from "./EntryName";
import { isLockedFolder, ItemIcon } from "./ItemIcon";
import { dropStateOf, itemProps } from "./itemProps";
import { folderOf, recordingDay } from "./meta";
import { StatusCell } from "./StatusCell";

/** A row in the list view, on the table's `cols`: name (with the new dot,
 *  and in the flat list the folder it is in), Kind or Status, the day it
 *  last changed, its size. */
export function DriveRow({ item, cols, narrow }: { item: DriveItem; cols: string; narrow: boolean }) {
  const view = useDriveView();
  const drop = dropStateOf(item, view);
  const selected = view.selected.has(item.key);
  const where = view.flat ? folderOf(item.key, view.root) : "";
  const modified =
    item.type === "recording" ? recordingDay(item.entry.date)
      : item.entry.modified != null ? fmtRelativeDay(item.entry.modified) : "";
  return (
    <EntryMenu item={item}>
      <div
        {...itemProps(item, view)}
        className={cn(
          cols,
          DRAG_SURFACE,
          "cursor-default py-2 outline-none transition-[background-color] duration-100",
          drop === "ok" ? "bg-brand/10" : drop === "refused" ? "opacity-50" : selected ? "bg-brand/5" : "hover:bg-surface/60",
        )}
      >
        <div className="flex min-w-0 items-center gap-2.5">
          <ItemIcon item={item} size={15} locked={isLockedFolder(item, view.writable)} />
          {item.type === "file" && item.row && <RecencyDot file={item.row} />}
          <div className="flex min-w-0 flex-1 items-baseline gap-2 text-[12.5px] text-foreground">
            <EntryName item={item} />
            {where && <span className="shrink-0 truncate text-[11px] text-muted-foreground max-w-[40%]">{where}</span>}
          </div>
          {drop && (
            <span className={cn("shrink-0 text-[11px]", drop === "ok" ? "text-brand" : "text-muted-foreground")}>
              {drop === "ok" ? "Drop to add" : "Can't add here"}
            </span>
          )}
        </div>
        {!narrow && <StatusCell item={item} />}
        <span className="text-right text-[11.5px] text-muted-foreground tabular-nums">{modified}</span>
        {!narrow && (
          <span className="text-right text-[11.5px] text-muted-foreground tabular-nums">
            {item.type === "file" ? fmtSize(item.entry.size) : ""}
          </span>
        )}
      </div>
    </EntryMenu>
  );
}
