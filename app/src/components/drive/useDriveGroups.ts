import { useMemo } from "react";
import type { DbFile, Subject } from "@/lib/db";
import { filterListing, type DriveFilters } from "@/lib/drive/filters";
import { fileItem, folderItem, recordingItem, type DriveItem } from "@/lib/drive/items";
import type { FolderListing } from "@/lib/drive/listing";
import { subjectRelOf } from "@/lib/drive/paths";
import { isFolded, type DriveGroup, type GroupFolds } from "./content/groups";

/**
 * The listing as groups of items, and the order they show in (what ⇧-click
 * ranges and ⌘A follow). At the Drive root, subjects split into the current
 * term and Past subjects; a folder of no known subject counts as past.
 * Everywhere else one group: folders, recordings, files. In the flat list
 * the filters narrow it first.
 */
export function useDriveGroups({ listing, rows, subjects, filters, flat, folds }: {
  listing: FolderListing | null;
  rows: ReadonlyMap<string, DbFile>;
  subjects: readonly Subject[];
  filters: DriveFilters;
  flat: boolean;
  folds: GroupFolds;
}): { groups: DriveGroup[]; order: DriveItem[] } {
  const groups = useMemo<DriveGroup[]>(() => {
    if (!listing) return [];
    const atRoot = listing.rel === "";
    if (atRoot && !flat) {
      const subjectsHere = listing.folders.map((f) => folderItem(f, subjects, true));
      const current = subjectsHere.filter((i) => i.type === "folder" && i.subject?.is_current);
      const past = subjectsHere.filter((i) => !(i.type === "folder" && i.subject?.is_current));
      return [
        { key: "current", label: "Subjects", items: current, foldable: false },
        { key: "past", label: "Past subjects", items: past, foldable: true },
      ].filter((g) => g.items.length > 0);
    }
    const shown = flat ? filterListing(listing, rows, filters, new Date()) : listing;
    const items: DriveItem[] = [
      ...shown.folders.map((f) => folderItem(f, subjects, subjectRelOf(f.relativePath) === f.relativePath)),
      ...(flat ? [] : listing.recordings.map(recordingItem)),
      ...shown.files.map((f) => fileItem(f, rows)),
    ];
    return [{ key: "all", label: null, items, foldable: false }];
  }, [listing, rows, subjects, filters, flat]);

  const order = useMemo(
    () => groups.flatMap((g) => (isFolded(g, folds) ? [] : g.items)),
    [groups, folds],
  );
  return { groups, order };
}
