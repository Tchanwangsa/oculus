import { useRef, type ReactNode } from "react";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import type { DriveItem } from "@/lib/drive/items";
import { FileCard } from "./FileCard";
import { FolderCard } from "./FolderCard";
import { GroupHeading } from "./GroupHeading";
import { isFolded, type DriveGroup, type GroupFolds } from "./groups";
import { useGridGlide } from "./useGridGlide";

/** Folder cards keep one width whatever the pane's, so the details panel
 *  opening never moves what sits at their end (the lock). */
const FOLDER_GRID = "grid grid-cols-[repeat(auto-fill,220px)] gap-2.5";
const FILE_GRID = "grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-2.5";

/**
 * The grid view: per group, folder cards, then recordings and files as
 * picture cards, the two runs told apart by their shape. `before` sits above the
 * cards (the read-only line, a problem) and scrolls with them. When the
 * details panel takes or gives back width, the cards glide to their new
 * columns (`useGridGlide`).
 */
export function DriveGrid({ groups, folds, before, empty }: {
  groups: DriveGroup[];
  folds: GroupFolds;
  before?: ReactNode;
  empty?: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useScrollFade(ref, "y");
  useGridGlide(ref);
  return (
    <div ref={ref} className="relative min-h-0 flex-1 overflow-y-scroll px-5 pb-6">
      {before}
      {empty}
      {groups.map((group) => (
        <section key={group.key} className="mt-4 first:mt-2">
          <div data-glide>
            <GroupHeading group={group} folds={folds} className="mb-2" />
          </div>
          {!isFolded(group, folds) && <GroupCards group={group} />}
        </section>
      ))}
    </div>
  );
}

function GroupCards({ group }: { group: DriveGroup }) {
  const folders = group.items.filter((i) => i.type === "folder");
  const pictures = group.items.filter((i) => i.type !== "folder");
  return (
    <>
      <Run items={folders} grid={FOLDER_GRID} card={(i) => <FolderCard key={i.key} item={i} />} />
      <Run items={pictures} grid={FILE_GRID} card={(i) => <FileCard key={i.key} item={i} />} spaced={folders.length > 0} />
    </>
  );
}

function Run({ items, grid, card, spaced = false }: {
  items: DriveItem[];
  grid: string;
  card: (item: DriveItem) => ReactNode;
  spaced?: boolean;
}) {
  if (!items.length) return null;
  return (
    <div className={spaced ? "mt-5" : undefined}>
      <div role="listbox" aria-multiselectable className={grid}>{items.map(card)}</div>
    </div>
  );
}
