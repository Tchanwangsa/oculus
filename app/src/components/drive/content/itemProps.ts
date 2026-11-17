import type { HTMLAttributes } from "react";
import type { DriveItem } from "@/lib/drive/items";
import type { DriveView } from "../DriveContext";

/** How a card or row stands relative to the drag in progress. */
export type DropState = "ok" | "refused" | null;

export function dropStateOf(item: DriveItem, view: DriveView): DropState {
  if (item.type !== "folder" || view.hover?.rel !== item.key) return null;
  return view.hover.ok ? "ok" : "refused";
}

/**
 * The props every card and row carries: selection on click (a drag never
 * selects text first), opening on double-click, the right-click picking the
 * entry first as Finder does, an HTML5 drag, and for a folder the drop
 * target attributes `useDriveDrop` reads. No `data-tab-href`: ⌘-click
 * toggles the selection here, and a new tab is in the menu.
 */
export function itemProps(item: DriveItem, view: DriveView): HTMLAttributes<HTMLDivElement> & Record<`data-${string}`, string | undefined> {
  const selected = view.selected.has(item.key);
  const renaming = view.renaming === item.key;
  return {
    role: "option",
    "aria-selected": selected,
    "data-drive-item": item.key,
    "data-drop-rel": item.type === "folder" ? item.key : undefined,
    "data-drop-writable": item.type === "folder" ? String(item.entry.writable) : undefined,
    draggable: item.type !== "recording" && !(item.type === "folder" && item.subject) && !renaming,
    onClick: (e) => view.click(item, e),
    onDoubleClick: () => view.open(item),
    onContextMenu: (e) => {
      if (!selected) view.click(item, { ...e, metaKey: false, ctrlKey: false, shiftKey: false } as typeof e);
    },
    onDragStart: (e) => view.dragStart(item, e),
  };
}
