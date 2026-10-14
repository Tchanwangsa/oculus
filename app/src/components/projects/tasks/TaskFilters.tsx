import type { ReactNode } from "react";
import { FunnelSimple } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { ChipRow, ToggleChip } from "@/components/ui/filter/ChipRow";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import type { DbTaskTag } from "@/lib/planning/projects";
import { TagDot } from "../tags/TagDot";
import { DueRangeFields } from "./DueRange";
import {
  DEFAULT_FILTER,
  popoverFilterCount,
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

function Section({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <div className="py-2">
      <div className="mb-1.5 flex items-baseline justify-between px-0.5">
        <span className="text-[11px] font-medium text-muted-foreground">{title}</span>
        {hint && <span className="text-[11px] text-muted-foreground/60">{hint}</span>}
      </div>
      {children}
    </div>
  );
}

/**
 * The Filter pill and the popover behind it: status, due and tags, each a
 * single line of chips (Due and Tags scroll sideways), at a fixed width so
 * nothing resizes it. Project and subject are their own toolbar pills
 * (`ProjectFilter`, `SubjectFilter`). The page holds the state and
 * `filterRules.ts` gives it meaning; a count badge says how many sections are
 * narrowing. Picking never closes the popover, so several can be set at once.
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
  const count = popoverFilterCount(filter);

  // The last checked status can't be unchecked — see `TaskFilter.status`.
  const toggleStatus = (id: string) => {
    const on = filter.status.includes(id);
    if (on && filter.status.length === 1) return;
    onChange({
      ...filter,
      status: UNIVERSAL_COLUMNS.filter((c) => (c.id === id ? !on : filter.status.includes(c.id))).map((c) => c.id),
    });
  };

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
            <span className="flex h-4 min-w-4 items-center justify-center rounded-full bg-primary px-1 text-[10px] font-medium tabular-nums leading-none text-primary-foreground">
              {count}
            </span>
          )}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-80 p-0">
        <div className="flex h-10 items-center justify-between px-4">
          <h2 className="font-display text-[13px] font-semibold text-foreground">Filter</h2>
          {count > 0 && (
            <button
              type="button"
              onClick={() => onChange({ ...DEFAULT_FILTER, project: filter.project, subject: filter.subject })}
              className="cursor-pointer rounded text-[11px] text-muted-foreground outline-none hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50"
            >
              Clear all
            </button>
          )}
        </div>
        <div className="px-4 pb-2">
          <Section title="Status">
            <div className="flex flex-wrap gap-1.5">
              {UNIVERSAL_COLUMNS.map((c) => (
                <ToggleChip key={c.id} selected={filter.status.includes(c.id)} onClick={() => toggleStatus(c.id)}>
                  {c.name}
                </ToggleChip>
              ))}
            </div>
          </Section>

          <Section title="Due">
            <ChipRow>
              {(Object.keys(DUE_LABELS) as DueFilter[]).map((d) => (
                <ToggleChip key={d} selected={filter.due === d} onClick={() => onChange({ ...filter, due: d })}>
                  {DUE_LABELS[d]}
                </ToggleChip>
              ))}
            </ChipRow>
            {filter.due === "custom" && (
              <DueRangeFields range={filter.dueRange} onChange={(dueRange) => onChange({ ...filter, dueRange })} />
            )}
          </Section>

          <Section title="Tags" hint={tags.length > 0 ? "Any of" : undefined}>
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
        </div>
      </PopoverContent>
    </Popover>
  );
}
