import { useMemo, useState } from "react";
import { CaretRight, Check, Kanban, Tray } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode } from "@/lib/format/format";
import type { DbProject } from "@/lib/planning/projects";

/**
 * Where a task lives: Unfiled or one project, archived ones behind a fold.
 * A plain list, not a `Select` — that would be a portal inside the popover.
 */
export function ProjectDestinations({
  projects,
  value,
  onPick,
  unfiledHint,
  className,
}: {
  /** Archived ones included (`status: "all"`). */
  projects: DbProject[];
  /** `null` is unfiled. */
  value: number | null;
  onPick: (projectId: number | null) => void;
  unfiledHint?: string;
  className?: string;
}) {
  const { active, archived } = useMemo(
    () => ({
      active: projects.filter((p) => p.status !== "archived"),
      archived: projects.filter((p) => p.status === "archived"),
    }),
    [projects],
  );
  const [archivedOpen, setArchivedOpen] = useState(false);

  // The fold never hides the current selection.
  const archivedSelected = archived.some((p) => p.id === value);

  return (
    <div className={cn("-mx-1 max-h-52 overflow-y-auto px-1", className)}>
      <DestinationRow
        label="Unfiled"
        hint={unfiledHint ?? "Not part of any project"}
        project={null}
        selected={value == null}
        onPick={() => onPick(null)}
      />
      {active.map((p) => (
        <DestinationRow
          key={p.id}
          label={p.name}
          project={p}
          selected={p.id === value}
          onPick={() => onPick(p.id)}
        />
      ))}

      {archived.length > 0 && (
        <Collapsible
          open={archivedOpen || archivedSelected}
          onOpenChange={setArchivedOpen}
          className="mt-1"
        >
          <CollapsibleTrigger className="flex w-full cursor-pointer items-center gap-1.5 rounded-md px-2 py-1.5 text-left text-[11px] font-medium text-muted-foreground transition-colors hover:text-foreground">
            <CaretRight
              size={9}
              className={cn(
                "shrink-0 transition-transform",
                (archivedOpen || archivedSelected) && "rotate-90",
              )}
            />
            Archived ({archived.length})
          </CollapsibleTrigger>
          <CollapsibleContent>
            {archived.map((p) => (
              <DestinationRow
                key={p.id}
                label={p.name}
                project={p}
                selected={p.id === value}
                onPick={() => onPick(p.id)}
              />
            ))}
          </CollapsibleContent>
        </Collapsible>
      )}
    </div>
  );
}

/** {@link ProjectDestinations} in a popover; picking closes it and writes. */
export function ProjectPicker({
  projects,
  value,
  onPick,
  label,
  unfiledHint,
  align = "start",
  children,
}: {
  projects: DbProject[];
  value: number | null;
  onPick: (projectId: number | null) => void;
  label?: string;
  unfiledHint?: string;
  align?: "start" | "center" | "end";
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>{children}</PopoverTrigger>
      <PopoverContent align={align} className="w-64 p-2">
        {label && (
          <p className="mt-1 mb-1 px-1 text-[11px] font-medium tracking-wide text-muted-foreground">
            {label}
          </p>
        )}
        <ProjectDestinations
          projects={projects}
          value={value}
          unfiledHint={unfiledHint}
          onPick={(projectId) => {
            setOpen(false);
            if (projectId !== value) onPick(projectId);
          }}
        />
      </PopoverContent>
    </Popover>
  );
}

function DestinationRow({
  label,
  hint,
  project,
  selected,
  onPick,
}: {
  label: string;
  hint?: string;
  /** `null` is the unfiled row. */
  project: DbProject | null;
  selected: boolean;
  onPick: () => void;
}) {
  return (
    <button
      type="button"
      title={hint ?? label}
      onClick={onPick}
      className={cn(
        "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[12.5px] transition-colors",
        selected
          ? "bg-accent text-foreground"
          : "text-muted-foreground hover:bg-accent hover:text-foreground",
      )}
    >
      {project == null ? (
        <Tray size={13} className="shrink-0" />
      ) : project.subject_code ? (
        <SubjectIcon code={project.subject_code} size={13} />
      ) : (
        <Kanban size={13} className="shrink-0" />
      )}
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {project?.subject_code && (
        <span className="shrink-0 text-[11px] text-muted-foreground/70">
          {displayCode(project.subject_code)}
        </span>
      )}
      {selected && <Check size={12} weight="bold" className="shrink-0 text-brand" />}
    </button>
  );
}
