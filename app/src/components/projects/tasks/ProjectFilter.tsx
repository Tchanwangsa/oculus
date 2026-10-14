import { useMemo } from "react";
import { Kanban, Tray } from "@phosphor-icons/react";
import { FilterFold } from "@/components/ui/filter/FilterFold";
import { FilterList } from "@/components/ui/filter/FilterList";
import { FilterPopover, FilterRow } from "@/components/ui/filter/FilterPopover";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import type { DbProject } from "@/lib/planning/projects";
import { TOOLBAR_PILL } from "./toolbarPill";
import type { TaskFilter } from "./filterRules";

function ProjectRow({
  project,
  selected,
  onPick,
}: {
  project: DbProject;
  selected: boolean;
  onPick: () => void;
}) {
  return (
    <FilterRow
      label={project.name}
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
 * The Project pill: Any, Unfiled or one project. Archived projects sit in a
 * fold, open while one of them is picked. Picking closes the popover.
 */
export function ProjectFilter({
  filter,
  projects,
  onChange,
}: {
  filter: TaskFilter;
  /** Archived included. */
  projects: DbProject[];
  onChange: (next: TaskFilter) => void;
}) {
  const { active, archived } = useMemo(
    () => ({
      active: projects.filter((p) => p.status !== "archived"),
      archived: projects.filter((p) => p.status === "archived"),
    }),
    [projects],
  );

  const value = filter.project;
  const label =
    value === "any" ? "Project" : value === "unfiled" ? "Unfiled" : (projects.find((p) => p.id === value)?.name ?? "Project");
  const Glyph = value === "unfiled" ? Tray : Kanban;

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
          onChange({ ...filter, project });
          close();
        };
        return (
          <FilterList>
            <FilterRow label="Any project" selected={value === "any"} onPick={() => pick("any")} />
            <FilterRow
              label="Unfiled"
              icon={<Tray size={13} className="shrink-0" />}
              selected={value === "unfiled"}
              onPick={() => pick("unfiled")}
            />
            {active.map((p) => (
              <ProjectRow key={p.id} project={p} selected={value === p.id} onPick={() => pick(p.id)} />
            ))}
            {archived.length > 0 && (
              <FilterFold label="Archived" count={archived.length} forceOpen={archived.some((p) => p.id === value)}>
                {archived.map((p) => (
                  <ProjectRow key={p.id} project={p} selected={value === p.id} onPick={() => pick(p.id)} />
                ))}
              </FilterFold>
            )}
          </FilterList>
        );
      }}
    </FilterPopover>
  );
}
