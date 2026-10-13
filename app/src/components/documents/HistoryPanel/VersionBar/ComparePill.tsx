import { useId } from "react";

import { FilterPopover, FilterRow } from "@/components/ui/filter/FilterPopover";
import { Switch } from "@/components/ui/switch";

import type { CompareWith } from "../useCompare";
import { compareLabel } from "./labels";

/**
 * What the marks compare against, as a filter pill ("vs previous", "vs
 * current note", "Changes hidden") over a popover: Show changes, then the
 * two bases. Picking a base turns changes back on. Previous is off for the
 * oldest version, which has none.
 */
export function ComparePill({
  showChanges,
  onShowChanges,
  compare,
  onCompare,
  previousDisabled,
}: {
  showChanges: boolean;
  onShowChanges: (on: boolean) => void;
  /** As applied: Current note for the oldest version whatever was chosen. */
  compare: CompareWith;
  onCompare: (value: CompareWith) => void;
  previousDisabled: boolean;
}) {
  const switchId = useId();
  const pick = (value: CompareWith, close: () => void) => {
    onCompare(value);
    onShowChanges(true);
    close();
  };
  return (
    <FilterPopover label={compareLabel(showChanges, compare)} active={showChanges} className="w-60">
      {(close) => (
        <>
          <label
            htmlFor={switchId}
            className="flex cursor-pointer items-center justify-between gap-2 rounded-md px-2 py-1.5 text-[12.5px] text-foreground"
          >
            Show changes
            <Switch id={switchId} size="sm" checked={showChanges} onCheckedChange={onShowChanges} />
          </label>
          <div className="my-1 h-px bg-border-subtle" />
          <p className="px-2 pt-0.5 pb-1 text-[11px] text-muted-foreground">Compared with</p>
          <FilterRow
            label="Previous version"
            selected={showChanges && compare === "previous"}
            disabled={previousDisabled}
            onPick={() => pick("previous", close)}
          />
          <FilterRow
            label="Current note"
            selected={showChanges && compare === "current"}
            onPick={() => pick("current", close)}
          />
          {previousDisabled && (
            <p className="px-2 pt-1 pb-0.5 text-[11px] text-muted-foreground">
              This is the oldest version, so there is no previous one.
            </p>
          )}
        </>
      )}
    </FilterPopover>
  );
}
