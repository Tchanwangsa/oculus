import type { DriveItem } from "@/lib/drive/items";

/** A run of entries under one heading: at the Drive root the current and
 *  past subjects; inside a folder one unlabelled group. A `foldable` group
 *  starts folded (Past subjects) until opened. */
export interface DriveGroup {
  key: string;
  label: string | null;
  items: DriveItem[];
  foldable: boolean;
}

export interface GroupFolds {
  open: ReadonlySet<string>;
  toggle: (key: string) => void;
}

export const isFolded = (group: DriveGroup, folds: GroupFolds) => group.foldable && !folds.open.has(group.key);
