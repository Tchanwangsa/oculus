import { createContext, useContext, type DragEvent, type MouseEvent } from "react";
import type { DropHover } from "@/hooks/drive/useDriveDrop";
import type { DriveItem } from "@/lib/drive/items";

/**
 * What every card, row and menu in one `DriveBrowser` shares: the selection,
 * the rename in progress, the drop hover, and the actions an entry offers.
 */
export interface DriveView {
  selected: ReadonlySet<string>;
  /** A click on an entry, with its modifiers (⌘ toggles, ⇧ extends). */
  click: (item: DriveItem, e: MouseEvent) => void;
  open: (item: DriveItem) => void;
  openInNewTab: (item: DriveItem) => void;
  /** `data-tab-href` for an entry: its ⌘-click route. */
  hrefOf: (item: DriveItem) => string | null;
  renaming: string | null;
  startRename: (item: DriveItem) => void;
  /** `null` cancels. */
  finishRename: (item: DriveItem, name: string | null) => void;
  dragStart: (item: DriveItem, e: DragEvent) => void;
  hover: DropHover | null;
  /** What a menu on `item` acts on: the selection when it holds `item`,
   *  else `item` alone. */
  targetsOf: (item: DriveItem) => DriveItem[];
  copyToMine: (items: DriveItem[]) => void;
  moveTo: (items: DriveItem[]) => void;
  remove: (items: DriveItem[]) => void;
  reveal: (item: DriveItem) => void;
  /** The listed folder takes new entries (its folders show a lock where
   *  they don't). */
  writable: boolean;
  /** The flat filtered list: entries show the folder they are in. */
  flat: boolean;
  /** The browser's root, for naming an entry's folder in the flat list. */
  root: string;
}

const DriveContext = createContext<DriveView | null>(null);

export const DriveViewProvider = DriveContext.Provider;

export function useDriveView(): DriveView {
  const view = useContext(DriveContext);
  if (!view) throw new Error("useDriveView outside a DriveBrowser");
  return view;
}
