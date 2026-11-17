import { LockSimple } from "@phosphor-icons/react";
import type { DriveItem } from "@/lib/drive/items";
import { displayName } from "@/lib/format/format";
import { cn } from "@/lib/utils";
import { useDriveView } from "../DriveContext";
import { EntryMenu } from "../menu/EntryMenu";
import { CARD, cardState } from "./cardStyle";
import { EntryName } from "./EntryName";
import { isLockedFolder, ItemIcon } from "./ItemIcon";
import { dropStateOf, itemProps } from "./itemProps";
import { entryMeta } from "./meta";

/** A folder or subject as a one-line card: glyph, name, and a line under it
 *  saying what it holds or, mid-drag, whether it takes the drop. A locked
 *  folder carries the lock at its end. */
export function FolderCard({ item }: { item: DriveItem }) {
  const view = useDriveView();
  const drop = dropStateOf(item, view);
  const locked = isLockedFolder(item, view.writable);
  const meta = drop === "ok" ? "Drop to add" : drop === "refused" ? "Can't add here" : entryMeta(item, view);
  const subject = item.type === "folder" ? item.subject : undefined;
  return (
    <EntryMenu item={item}>
      <div
        {...itemProps(item, view)}
        title={subject ? displayName(subject.name, subject.code) : item.name}
        className={cn(CARD, "flex items-center gap-2.5 px-3 py-2.5", cardState(view.selected.has(item.key), drop))}
      >
        <ItemIcon item={item} size={18} />
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 text-[12.5px] font-medium text-foreground">
            <EntryName item={item} />
          </div>
          <div className="truncate text-[11px] text-muted-foreground tabular-nums">{meta}</div>
        </div>
        {locked && <LockSimple size={13} weight="fill" className="shrink-0 text-muted-foreground/70" aria-label="Read-only" />}
      </div>
    </EntryMenu>
  );
}
