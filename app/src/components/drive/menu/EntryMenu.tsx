import type { ReactElement } from "react";
import { ArrowSquareOut, ArrowSquareUpRight, ArrowsOutCardinal, Copy, FolderOpen, LockSimple, Textbox, Trash } from "@phosphor-icons/react";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuNote,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { READ_ONLY_TEXT } from "@/lib/drive/commands";
import { itemEditable, itemZone, type DriveItem } from "@/lib/drive/items";
import { isMac } from "@/lib/platform";
import { useDriveView } from "../DriveContext";

const SHOW_IN = isMac ? "Show in Finder" : "Show in File Explorer";

/**
 * An entry's right-click menu. It acts on the selection when the entry is in
 * it. Everything that reads is always there; a synced entry adds Copy to my
 * files, and keeps Rename, Move and Delete greyed with the reason under them.
 */
export function EntryMenu({ item, children }: { item: DriveItem; children: ReactElement }) {
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      {/* The trigger must not take focus back from a rename's field. */}
      <ContextMenuContent onCloseAutoFocus={(e) => e.preventDefault()}>
        <MenuItems item={item} />
      </ContextMenuContent>
    </ContextMenu>
  );
}

function MenuItems({ item }: { item: DriveItem }) {
  const view = useDriveView();
  const targets = view.targetsOf(item);
  const one = targets.length === 1;
  const href = view.hrefOf(item);
  const synced = targets.filter((t) => t.type !== "recording" && itemZone(t) === "synced");
  // Recordings and subjects are never the student's to rename or delete, and
  // say nothing about it.
  const recordings = targets.some((t) => t.type === "recording" || (t.type === "folder" && t.subject));
  const editable = targets.every(itemEditable);
  const locked = !editable && !recordings;
  return (
    <>
      {one && <ContextMenuItem icon={<ArrowSquareUpRight size={14} />} onSelect={() => view.open(item)}>Open</ContextMenuItem>}
      {one && href && (
        <ContextMenuItem icon={<ArrowSquareOut size={14} />} onSelect={() => view.openInNewTab(item)}>
          Open in new tab
        </ContextMenuItem>
      )}
      {synced.length > 0 && !recordings && (
        <ContextMenuItem icon={<Copy size={14} />} onSelect={() => view.copyToMine(synced)}>
          Copy to my files…
        </ContextMenuItem>
      )}
      {one && <ContextMenuItem icon={<FolderOpen size={14} />} onSelect={() => view.reveal(item)}>{SHOW_IN}</ContextMenuItem>}
      {!recordings && (
        <>
          <ContextMenuSeparator />
          <ContextMenuItem
            icon={<Textbox size={14} />}
            disabled={!editable || !one}
            title={locked ? READ_ONLY_TEXT : undefined}
            onSelect={() => view.startRename(item)}
          >
            Rename
          </ContextMenuItem>
          <ContextMenuItem
            icon={<ArrowsOutCardinal size={14} />}
            disabled={!editable}
            title={locked ? READ_ONLY_TEXT : undefined}
            onSelect={() => view.moveTo(targets)}
          >
            Move to…
          </ContextMenuItem>
          <ContextMenuItem
            icon={<Trash size={14} />}
            destructive={editable}
            disabled={!editable}
            title={locked ? READ_ONLY_TEXT : undefined}
            onSelect={() => view.remove(targets)}
          >
            Delete
          </ContextMenuItem>
          {locked && <ContextMenuNote icon={<LockSimple size={13} />}>{READ_ONLY_TEXT}</ContextMenuNote>}
        </>
      )}
    </>
  );
}
