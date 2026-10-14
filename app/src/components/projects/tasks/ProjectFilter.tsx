import { useMemo } from "react";
import { Circle, Kanban } from "@phosphor-icons/react";
import { FilterFold } from "@/components/ui/filter/FilterFold";
import { FilterList } from "@/components/ui/filter/FilterList";
import { FilterPopover, FilterRow } from "@/components/ui/filter/FilterPopover";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import type { DbProject } from "@/lib/planning/projects";
import { TOOLBAR_PILL } from "./toolbarPill";
import type { TaskFilterCounts } from "./filterCounts";
import type { TaskFilter } from "./filterRules";

function ProjectRow({
  project,
  count,
  selected,
  onPick,
}: {
  project: DbProject;
  count: number;
  selected: boolean;
  onPick: () => void;
}) {
  return (
    <FilterRow
      label={project.name}
      count={count}
      icon={
        project.subject_code ? (
          <SubjectIcon code={project.subject_code} size={13} />
        ) : (
          <Kanban size={13} className="shrink-0" />
        )
      }
      selected={selected}
      onPick={onPick}
    />
  );
}

/**
 * The Project pill: Any, No project or one project. Archived projects sit in a
 * fold, initially open when one of them is picked. Picking closes the popover.
 */
export function ProjectFilter({
  filter,
  projects,
  counts,
  onChange,
}: {
  filter: TaskFilter;
  /** Archived included. */
  projects: DbProject[];
  counts: TaskFilterCounts;
  onChange: (next: TaskFilter) => void;
}) {
  const { active, archived } = useMemo(
    () => ({
      active: projects.filter((p) => p.status !== "archived" && (counts.projects.get(p.id) ?? 0) > 0),
      archived: projects.filter((p) => p.status === "archived" && (counts.projects.get(p.id) ?? 0) > 0),
    }),
    [projects, counts.projects],
  );

  const value = filter.project;
  const label =
    value === "any" ? "Project" : value === "unfiled" ? "No project" : (projects.find((p) => p.id === value)?.name ?? "Project");
  const Glyph = value === "unfiled" ? Circle : Kanban;

  return (
    <FilterPopover
      label={label}
      active={value !== "any"}
      icon={<Glyph size={13} className={value === "any" ? "text-muted-foreground" : undefined} />}
      className="w-64"
      triggerClassName={TOOLBAR_PILL}
    >
      {(close) => {
        const pick = (project: TaskFilter["project"]) => {
          const next = project === value ? "any" : project;
          onChange({
            ...filter,
            project: next,
            showArchived: typeof next === "number" && archived.some((p) => p.id === next) ? true : filter.showArchived,
          });
          close();
        };
        return (
          <FilterList>
            <FilterRow count={counts.anyProject} label="Any project" selected={value === "any"} onPick={() => pick("any")} />
            {(counts.projects.get(null) ?? 0) > 0 && <FilterRow
              count={counts.projects.get(null) ?? 0}
              label="No project"
              icon={<Circle size={13} className="shrink-0" />}
              selected={value === "unfiled"}
              onPick={() => pick("unfiled")}
            />}
            {active.map((p) => (
              <ProjectRow key={p.id} project={p} count={counts.projects.get(p.id) ?? 0} selected={value === p.id} onPick={() => pick(p.id)} />
            ))}
            {archived.length > 0 && (
              <FilterFold label="Archived" count={archived.length} defaultOpen={archived.some((p) => p.id === value)}>
                {archived.map((p) => (
                  <ProjectRow key={p.id} project={p} count={counts.projects.get(p.id) ?? 0} selected={value === p.id} onPick={() => pick(p.id)} />
                ))}
              </FilterFold>
            )}
          </FilterList>
        );
      }}
    </FilterPopover>
  );
}
