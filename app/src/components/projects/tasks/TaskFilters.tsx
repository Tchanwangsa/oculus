import type { ReactNode } from "react";
import { FunnelSimple } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { ChipRow, ToggleChip } from "@/components/ui/filter/ChipRow";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import type { DbTaskTag } from "@/lib/planning/projects";
import { TagDot } from "../tags/TagDot";
import { DueRangeFields } from "./DueRange";
import {
  DEFAULT_FILTER,
  isDefaultFilter,
  activeFilterCount,
  toggleStatusFilter,
  type DueFilter,
  type TaskFilter,
} from "./filterRules";
import { UNIVERSAL_COLUMNS } from "./universalTasks";

const DUE_LABELS: Record<DueFilter, string> = {
  any: "Any time",
  overdue: "Overdue",
  today: "Today",
  week: "This week",
  undated: "Undated",
  custom: "Custom",
};

function Section({ title, action, children }: { title: string; action?: ReactNode; children: ReactNode }) {
  return (
    <div className="py-2">
      <div className="mb-1.5 flex items-baseline justify-between px-0.5">
        <span className="text-[11px] font-medium text-muted-foreground">{title}</span>
        {action}
      </div>
      {children}
    </div>
  );
}

function SelectionAction({ changed, label, onClick, disabled = false }: {
  changed: boolean;
  label: string;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      aria-label={changed ? `Reset ${label}` : `Select all ${label}`}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "cursor-pointer rounded text-[11px] outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-default disabled:opacity-50",
        changed ? "text-brand hover:text-brand/80" : "text-muted-foreground hover:text-foreground",
      )}
    >
      {changed ? "Reset" : "Select all"}
    </button>
  );
}

/**
 * The Filter pill and the popover behind it: archived visibility, status, due and tags, with a
 * row of chips (Due and Tags scroll sideways), at a fixed width so
 * nothing resizes it. Project and subject are their own toolbar pills
 * (`ProjectFilter`, `SubjectFilter`). The page holds the state and
 * `filterRules.ts` gives it meaning; the pill counts sections changed from
 * default. Picking never closes the popover, so several can be set at once.
 */
export function TaskFilters({
  filter,
  tags,
  onChange,
}: {
  filter: TaskFilter;
  tags: DbTaskTag[];
  onChange: (next: TaskFilter) => void;
}) {
  const count = activeFilterCount(filter);

  const toggleStatus = (id: string) =>
    onChange({ ...filter, status: toggleStatusFilter(filter.status, id) });

  const toggleTag = (id: number) =>
    onChange({
      ...filter,
      tags: filter.tags.includes(id) ? filter.tags.filter((t) => t !== id) : [...filter.tags, id],
    });

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          size="sm"
          className={cn("shrink-0 gap-1.5 font-normal", count > 0 && "border-brand/40 text-brand")}
        >
          <FunnelSimple size={13} className={count > 0 ? undefined : "text-muted-foreground"} />
          Filter
          {count > 0 && (
            <span className="tabular-nums">
              ({count})
            </span>
          )}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-80 p-0">
        <div className="px-4 py-3">
          <div className="flex min-h-9 items-center justify-between gap-3">
            <span className="text-xs text-foreground">Show archived</span>
            <Switch aria-label="Show tasks in archived projects" checked={filter.showArchived} onCheckedChange={(showArchived) => onChange({ ...filter, showArchived })} />
          </div>
          <Section title="Status" action={
            <SelectionAction
              changed={filter.status.length > 0}
              label="statuses"
              onClick={() => onChange({
                ...filter,
                status: filter.status.length > 0 ? DEFAULT_FILTER.status : UNIVERSAL_COLUMNS.map((c) => c.id),
              })}
            />
          }>
            <ChipRow>
              {UNIVERSAL_COLUMNS.map((c) => (
                <ToggleChip key={c.id} selected={filter.status.includes(c.id)} onClick={() => toggleStatus(c.id)}>
                  {c.name}
                </ToggleChip>
              ))}
            </ChipRow>
          </Section>

          <Section title="Due">
            <ChipRow>
              {(Object.keys(DUE_LABELS) as DueFilter[]).map((d) => (
                <ToggleChip key={d} selected={filter.due === d} onClick={() => onChange({ ...filter, due: filter.due === d ? "any" : d })}>
                  {DUE_LABELS[d]}
                </ToggleChip>
              ))}
            </ChipRow>
            {filter.due === "custom" && (
              <DueRangeFields range={filter.dueRange} onChange={(dueRange) => onChange({ ...filter, dueRange })} />
            )}
          </Section>

          <Section title="Tags" action={
            <SelectionAction
              changed={filter.tags.length > 0}
              label="tags"
              disabled={tags.length === 0 && filter.tags.length === 0}
              onClick={() => onChange({
                ...filter,
                tags: filter.tags.length > 0 ? DEFAULT_FILTER.tags : tags.map((tag) => tag.id),
              })}
            />
          }>
            {tags.length === 0 ? (
              <span className="px-0.5 text-xs text-muted-foreground/70">No tags yet.</span>
            ) : (
              <ChipRow>
                {tags.map((tag) => (
                  <ToggleChip key={tag.id} selected={filter.tags.includes(tag.id)} onClick={() => toggleTag(tag.id)}>
                    <TagDot color={tag.color} />
                    {tag.name}
                  </ToggleChip>
                ))}
              </ChipRow>
            )}
          </Section>
          <div className="mt-1.5 border-t border-border-subtle pt-2 pl-0.5">
            <button
              type="button"
              disabled={isDefaultFilter(filter)}
              onClick={() => onChange(DEFAULT_FILTER)}
              className="cursor-pointer rounded text-[11px] text-muted-foreground outline-none hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-default disabled:opacity-50"
            >
              Reset defaults
            </button>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  );
}
