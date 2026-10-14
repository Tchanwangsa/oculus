import { useMemo, useRef, useState, type ReactNode } from "react";
import { FunnelSimple, Kanban, Tray } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { CollapsibleSection } from "@/components/ui/CollapsibleSection";
import { FilterRow } from "@/components/ui/filter/FilterPopover";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { useSubjects } from "@/hooks/data/useSubjects";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { displayCode } from "@/lib/format/format";
import type { DbProject, DbTaskTag } from "@/lib/planning/projects";
import { TagDot } from "../tags/TagDot";
import {
  DEFAULT_FILTER,
  activeFilterCount,
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
};

function Chip({
  selected,
  onClick,
  children,
}: {
  selected: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-pressed={selected}
      onClick={onClick}
      className={cn(
        "inline-flex h-7 max-w-full min-w-0 cursor-pointer items-center gap-1.5 rounded-full border px-2.5 text-xs outline-none transition-[color,background-color,border-color]",
        "focus-visible:ring-[3px] focus-visible:ring-ring/50",
        selected
          ? "border-brand/40 bg-brand-muted text-brand"
          : "border-border text-muted-foreground hover:bg-accent hover:text-foreground",
      )}
    >
      {children}
    </button>
  );
}

function Section({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <div className="py-2">
      <div className="mb-1.5 flex items-baseline justify-between px-0.5">
        <span className="text-[11px] font-medium text-muted-foreground">{title}</span>
        {hint && <span className="text-[11px] text-muted-foreground/60">{hint}</span>}
      </div>
      <div className="flex flex-wrap gap-1.5">{children}</div>
    </div>
  );
}

/**
 * The Filter pill and the popover behind it: status, due and tags as chips,
 * project and subject as folded lists. The page holds the state and
 * `filterRules.ts` gives it meaning; a count badge says how many sections are
 * narrowing. Picking never closes the popover, so several can be set at once.
 */
export function TaskFilters({
  filter,
  projects,
  tags,
  onChange,
}: {
  filter: TaskFilter;
  /** Archived included. */
  projects: DbProject[];
  tags: DbTaskTag[];
  onChange: (next: TaskFilter) => void;
}) {
  const { subjects } = useSubjects();
  const [projectOpen, setProjectOpen] = useState(false);
  const [subjectOpen, setSubjectOpen] = useState(false);
  const scroller = useRef<HTMLDivElement>(null);
  useScrollFade(scroller, "y");
  const count = activeFilterCount(filter);

  const projectName = useMemo(() => {
    if (filter.project === "any") return "Any";
    if (filter.project === "unfiled") return "Unfiled";
    return projects.find((p) => p.id === filter.project)?.name ?? "Project";
  }, [filter.project, projects]);

  const subjectName =
    filter.subject === "any" ? "Any" : filter.subject === "personal" ? "Personal" : displayCode(filter.subject);

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
              onClick={() => onChange(DEFAULT_FILTER)}
              className="cursor-pointer rounded text-[11px] text-muted-foreground outline-none hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50"
            >
              Clear all
            </button>
          )}
        </div>
        <div
          ref={scroller}
          className="max-h-[min(30rem,var(--radix-popover-content-available-height))] overflow-y-auto px-4 pb-2"
        >
          <Section title="Status">
            {UNIVERSAL_COLUMNS.map((c) => (
              <Chip key={c.id} selected={filter.status.includes(c.id)} onClick={() => toggleStatus(c.id)}>
                {c.name}
              </Chip>
            ))}
          </Section>

          <Section title="Due">
            {(Object.keys(DUE_LABELS) as DueFilter[]).map((d) => (
              <Chip key={d} selected={filter.due === d} onClick={() => onChange({ ...filter, due: d })}>
                {DUE_LABELS[d]}
              </Chip>
            ))}
          </Section>

          <Section title="Tags" hint={tags.length > 0 ? "Any of" : undefined}>
            {tags.length === 0 && <span className="px-0.5 text-xs text-muted-foreground/70">No tags yet.</span>}
            {tags.map((tag) => (
              <Chip key={tag.id} selected={filter.tags.includes(tag.id)} onClick={() => toggleTag(tag.id)}>
                <TagDot color={tag.color} />
                <span className="min-w-0 truncate">{tag.name}</span>
              </Chip>
            ))}
          </Section>

          <CollapsibleSection title="Project" open={projectOpen} onOpenChange={setProjectOpen} count={projectName}>
            <div className="-mx-1 max-h-40 overflow-y-auto px-1">
              <FilterRow
                label="Any project"
                selected={filter.project === "any"}
                onPick={() => onChange({ ...filter, project: "any" })}
              />
              <FilterRow
                label="Unfiled"
                icon={<Tray size={13} className="shrink-0" />}
                selected={filter.project === "unfiled"}
                onPick={() => onChange({ ...filter, project: "unfiled" })}
              />
              {projects.map((p) => (
                <FilterRow
                  key={p.id}
                  label={p.name}
                  icon={
                    p.subject_code ? (
                      <SubjectIcon code={p.subject_code} size={13} />
                    ) : (
                      <Kanban size={13} className="shrink-0" />
                    )
                  }
                  selected={filter.project === p.id}
                  onPick={() => onChange({ ...filter, project: p.id })}
                />
              ))}
            </div>
          </CollapsibleSection>

          <CollapsibleSection title="Subject" open={subjectOpen} onOpenChange={setSubjectOpen} count={subjectName}>
            <div className="-mx-1 max-h-40 overflow-y-auto px-1">
              <FilterRow
                label="Any subject"
                selected={filter.subject === "any"}
                onPick={() => onChange({ ...filter, subject: "any" })}
              />
              <FilterRow
                label="Personal"
                hint="No subject — personal projects, and anything unfiled"
                selected={filter.subject === "personal"}
                onPick={() => onChange({ ...filter, subject: "personal" })}
              />
              {subjects.map((s) => (
                <FilterRow
                  key={s.id}
                  label={displayCode(s.code)}
                  icon={<SubjectIcon code={s.code} size={13} />}
                  selected={filter.subject === s.code}
                  onPick={() => onChange({ ...filter, subject: s.code })}
                />
              ))}
            </div>
          </CollapsibleSection>
        </div>
      </PopoverContent>
    </Popover>
  );
}
