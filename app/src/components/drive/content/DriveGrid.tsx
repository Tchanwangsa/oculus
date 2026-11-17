import { useRef, type ReactNode } from "react";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import type { DriveItem } from "@/lib/drive/items";
import { FileCard } from "./FileCard";
import { FolderCard } from "./FolderCard";
import { GroupHeading } from "./GroupHeading";
import { isFolded, type DriveGroup, type GroupFolds } from "./groups";

const FOLDER_GRID = "grid grid-cols-[repeat(auto-fill,minmax(190px,1fr))] gap-2.5";
const FILE_GRID = "grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-2.5";

/**
 * The grid view: per group, folder cards, then recordings and files as
 * picture cards. Inside a folder the two runs get "Folders" and "Files"
 * headings, as the mockup's subject screen does. `before` sits above the
 * cards (the read-only line, a problem) and scrolls with them.
 */
export function DriveGrid({ groups, folds, before, empty }: {
  groups: DriveGroup[];
  folds: GroupFolds;
  before?: ReactNode;
  empty?: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useScrollFade(ref, "y");
  return (
    <div ref={ref} className="min-h-0 flex-1 overflow-y-scroll px-5 pb-6">
      {before}
      {empty}
      {groups.map((group) => (
        <section key={group.key} className="mt-4 first:mt-2">
          <GroupHeading group={group} folds={folds} className="mb-2" />
          {!isFolded(group, folds) && <GroupCards group={group} />}
        </section>
      ))}
    </div>
  );
}

function GroupCards({ group }: { group: DriveGroup }) {
  const folders = group.items.filter((i) => i.type === "folder");
  const pictures = group.items.filter((i) => i.type !== "folder");
  const titled = !group.label && folders.length > 0 && pictures.length > 0;
  return (
    <>
      <Run title={titled ? "Folders" : null} items={folders} grid={FOLDER_GRID} card={(i) => <FolderCard key={i.key} item={i} />} />
      <Run title={titled ? "Files" : null} items={pictures} grid={FILE_GRID} card={(i) => <FileCard key={i.key} item={i} />} spaced={folders.length > 0} />
    </>
  );
}

function Run({ title, items, grid, card, spaced = false }: {
  title: string | null;
  items: DriveItem[];
  grid: string;
  card: (item: DriveItem) => ReactNode;
  spaced?: boolean;
}) {
  if (!items.length) return null;
  return (
    <div className={spaced ? "mt-4" : undefined}>
      {title && <div className="mb-2 text-[11px] font-medium text-muted-foreground">{title}</div>}
      <div role="listbox" aria-multiselectable className={grid}>{items.map(card)}</div>
    </div>
  );
}
