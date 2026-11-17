import type { ReactNode } from "react";
import { GridTable, HeaderLabels } from "@/components/ui/table/GridTable";
import { DriveRow } from "./DriveRow";
import { GroupHeading } from "./GroupHeading";
import { isFolded, type DriveGroup, type GroupFolds } from "./groups";

const COLS = "grid grid-cols-[minmax(0,1fr)_128px_96px_72px] items-center gap-3 px-5";
const NARROW_COLS = "grid grid-cols-[minmax(0,1fr)_80px] items-center gap-3 px-5";

/**
 * The list view: `GridTable` in the Sync history's grammar, its header
 * outside the scroller and a group's heading as a band across the rows.
 * Columns are Name, Kind (Status in a synced folder, where it is the parse
 * state), Modified (Synced) and Size; a narrow pane keeps Name and the day.
 */
export function DriveList({ groups, folds, synced, narrow, before, empty }: {
  groups: DriveGroup[];
  folds: GroupFolds;
  synced: boolean;
  narrow: boolean;
  before?: ReactNode;
  empty?: ReactNode;
}) {
  const cols = narrow ? NARROW_COLS : COLS;
  const labels = narrow
    ? ["Name", synced ? "Synced" : "Modified"]
    : ["Name", synced ? "Status" : "Kind", synced ? "Synced" : "Modified", "Size"];
  return (
    <GridTable
      cols={cols}
      header={<HeaderLabels labels={labels} />}
      empty={<div className="px-5">{before}{empty}</div>}
    >
      <div role="listbox" aria-multiselectable className="divide-y divide-border-subtle">
        {groups.map((group) => (
          <div key={group.key}>
            {group.label && (
              <div className="border-b border-border-subtle bg-surface/70 px-5 py-1.5">
                <GroupHeading group={group} folds={folds} />
              </div>
            )}
            {!isFolded(group, folds) && (
              <div className="divide-y divide-border-subtle">
                {group.items.map((item) => <DriveRow key={item.key} item={item} cols={cols} narrow={narrow} />)}
              </div>
            )}
          </div>
        ))}
      </div>
    </GridTable>
  );
}
