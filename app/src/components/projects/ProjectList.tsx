import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { CaretRight, Plus } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode, displayName } from "@/lib/format";
import type { Subject } from "@/lib/db";
import type { DbProject, ProjectTaskCounts } from "@/lib/projects";
import { InlineAdd } from "./InlineAdd";
import { ProjectMenu } from "./ProjectMenu";
import { AgentMark, DueChip, ProjectProgress } from "./TaskMarks";
import { projectHref } from "./projectHref";
import { ListCard } from "@/components/ui/PageParts";

const COLLAPSED_KEY = "oculus-projects-groups-collapsed";

/** Stores the *collapsed* groups, so a new group arrives open. */
function loadCollapsed(): Set<string> {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? "[]");
    return new Set(Array.isArray(raw) ? raw.filter((k): k is string => typeof k === "string") : []);
  } catch {
    return new Set();
  }
}

/** A row's overflow-menu actions; omit to draw no menu. */
export interface ProjectRowActions {
  /** Trimmed and changed — {@link ProjectMenu} drops no-ops. */
  onRename: (project: DbProject, name: string) => void;
  onArchive: (project: DbProject) => void;
  onUnarchive: (project: DbProject) => void;
  onDelete: (project: DbProject) => void;
}

/** Groups by subject; subject-less or deleted-subject projects go under
 *  "Personal". */
function group(
  projects: DbProject[],
  subjects: Subject[],
): {
  key: string;
  label: string;
  title: string;
  subjectId: number | null;
  projects: DbProject[];
}[] {
  const out: ReturnType<typeof group> = [];
  const byKey = new Map<string, (typeof out)[number]>();
  for (const p of projects) {
    const subject =
      p.subject_id == null ? null : subjects.find((s) => s.id === p.subject_id) ?? null;
    const key = subject ? String(subject.id) : "personal";
    let g = byKey.get(key);
    if (!g) {
      g = {
        key,
        label: subject ? displayCode(subject.code) : "Personal",
        title: subject ? displayName(subject.name, subject.code) : "Not scoped to a subject",
        subjectId: subject?.id ?? null,
        projects: [],
      };
      byKey.set(key, g);
      out.push(g);
    }
    g.projects.push(p);
  }
  return out;
}

/** The menu is a sibling of the link, not inside it: a button nested in an
 *  anchor navigates on click. */
function ProjectRow({
  project,
  counts,
  actions,
  quiet = false,
}: {
  project: DbProject;
  counts: ProjectTaskCounts | undefined;
  actions?: ProjectRowActions;
  /** Archived styling: muted name, count instead of a progress bar. */
  quiet?: boolean;
}) {
  return (
    <div className="group/row flex items-center transition-colors hover:bg-surface">
      <Link
        to={projectHref(project)}
        className="flex min-w-0 flex-1 items-center gap-3 px-3 py-2.5"
      >
        <span className="min-w-0 flex-1">
          <span className="flex items-center gap-1.5">
            <span
              className={cn(
                "truncate text-[12px]",
                quiet ? "text-muted-foreground" : "text-foreground",
              )}
            >
              {project.name}
            </span>
            <AgentMark source={project.source} />
          </span>
          {project.brief && (
            <span className="mt-0.5 block truncate text-[11px] text-muted-foreground">
              {project.brief}
            </span>
          )}
        </span>
        {quiet ? (
          counts && counts.total > 0 ? (
            <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground/60">
              {counts.done}/{counts.total}
            </span>
          ) : null
        ) : (
          <ProjectProgress counts={counts} />
        )}
        <DueChip dueAt={project.due_at} />
        <CaretRight size={11} className="shrink-0 text-muted-foreground/40" />
      </Link>
      {actions && (
        <span className="shrink-0 pr-2 pl-0.5">
          <ProjectMenu
            project={project}
            onRename={(name) => actions.onRename(project, name)}
            onArchive={() => actions.onArchive(project)}
            onUnarchive={() => actions.onUnarchive(project)}
            onDelete={() => actions.onDelete(project)}
            /* Faded, not `hidden`, so it stays tabbable and anchors its
               open popover. */
            className="opacity-0 transition-opacity group-hover/row:opacity-100 focus-visible:opacity-100 data-[state=open]:opacity-100"
          />
        </span>
      )}
    </div>
  );
}

function RowBox({
  projects,
  counts,
  onCreate,
  addLabel,
  emptyCopy,
  actions,
  quiet = false,
  armed = false,
}: {
  projects: DbProject[];
  counts: Map<number, ProjectTaskCounts>;
  /** Omitted for the archived list. */
  onCreate?: (name: string) => void;
  addLabel?: string;
  emptyCopy: string;
  actions?: ProjectRowActions;
  quiet?: boolean;
  /** Open the composer focused; the `key` below remounts it so it re-arms. */
  armed?: boolean;
}) {
  return (
    <ListCard>
      {projects.length === 0 && (
        <p className="px-3 py-5 text-center text-xs text-muted-foreground">{emptyCopy}</p>
      )}
      {projects.map((p) => (
        <ProjectRow
          key={p.id}
          project={p}
          counts={counts.get(p.id)}
          actions={actions}
          quiet={quiet}
        />
      ))}
      {onCreate && (
        <div className="px-1.5 py-1">
          <InlineAdd
            key={armed ? "armed" : "idle"}
            defaultEditing={armed}
            label={addLabel ?? "New project"}
            placeholder="Project name"
            onAdd={onCreate}
          />
        </div>
      )}
    </ListCard>
  );
}

export function ProjectGroups({
  projects,
  counts,
  subjects,
  onCreate,
  actions,
}: {
  projects: DbProject[];
  counts: Map<number, ProjectTaskCounts>;
  subjects: Subject[];
  /** `null` is the Personal group. */
  onCreate: (subjectId: number | null, name: string) => void;
  actions?: ProjectRowActions;
}) {
  const [folded, setFolded] = useState<Set<string>>(loadCollapsed);
  const [arming, setArming] = useState<string | null>(null);
  const groups = useMemo(() => group(projects, subjects), [projects, subjects]);

  useEffect(() => {
    localStorage.setItem(COLLAPSED_KEY, JSON.stringify([...folded]));
  }, [folded]);

  const setOpen = (key: string, open: boolean) =>
    setFolded((prev) => {
      if (open === !prev.has(key)) return prev;
      const next = new Set(prev);
      if (open) next.delete(key);
      else next.add(key);
      return next;
    });

  // Personal is always shown, even empty, so it can be discovered.
  const shown = groups.some((g) => g.key === "personal")
    ? groups
    : [
        ...groups,
        {
          key: "personal",
          label: "Personal",
          title: "Not scoped to a subject",
          subjectId: null,
          projects: [],
        },
      ];

  return (
    <div className="space-y-5">
      {shown.map((g) => {
        const open = !folded.has(g.key);
        const subject = subjects.find((s) => s.id === g.subjectId) ?? null;
        return (
          <section key={g.key}>
            <div className="group/head mb-1.5 flex items-center gap-1.5 px-0.5">
              <button
                type="button"
                title={g.title}
                aria-expanded={open}
                onClick={() => setOpen(g.key, !open)}
                className="flex min-w-0 items-center gap-1.5 text-left transition-colors hover:opacity-70"
              >
                {subject && <SubjectIcon code={subject.code} size={12} />}
                {/* Not an <h2>: headings can't nest in a button. */}
                <span className="truncate font-display text-[13px] font-semibold tracking-tight text-foreground">
                  {g.label}
                </span>
                <CaretRight
                  size={11}
                  className={cn(
                    "shrink-0 text-muted-foreground transition-transform",
                    open && "rotate-90",
                  )}
                />
              </button>
              <span className="text-[11px] tabular-nums text-muted-foreground/60">
                {g.projects.length}
              </span>
              <span className="flex-1" />
              <button
                type="button"
                aria-label={`New project in ${g.label}`}
                title={`New project in ${g.label}`}
                onClick={() => {
                  setOpen(g.key, true);
                  setArming(g.key);
                }}
                className="hidden shrink-0 cursor-pointer rounded p-0.5 text-muted-foreground transition-colors hover:text-foreground group-hover/head:block"
              >
                <Plus size={12} weight="bold" />
              </button>
            </div>

            {open && (
              <RowBox
                projects={g.projects}
                counts={counts}
                actions={actions}
                armed={arming === g.key}
                onCreate={(name) => onCreate(g.subjectId, name)}
                addLabel="New project"
                emptyCopy={
                  g.subjectId == null
                    ? "Nothing personal on the go. Anything that isn't a subject's goes here."
                    : `No projects for ${g.label} yet.`
                }
              />
            )}
          </section>
        );
      })}
    </div>
  );
}

/** Stores *open* (the inverse of {@link COLLAPSED_KEY}): archived defaults
 *  shut. */
const ARCHIVED_OPEN_KEY = "oculus-projects-archived-open";

/** The caller renders this only when there are archived projects. */
export function ArchivedProjects({
  projects,
  counts,
  actions,
}: {
  projects: DbProject[];
  counts: Map<number, ProjectTaskCounts>;
  actions?: ProjectRowActions;
}) {
  const [open, setOpen] = useState(() => localStorage.getItem(ARCHIVED_OPEN_KEY) === "true");

  useEffect(() => {
    localStorage.setItem(ARCHIVED_OPEN_KEY, String(open));
  }, [open]);

  return (
    <section>
      <div className="mb-1.5 flex items-center gap-1.5 px-0.5">
        <button
          type="button"
          aria-expanded={open}
          onClick={() => setOpen((v) => !v)}
          className="flex min-w-0 items-center gap-1.5 text-left transition-colors hover:opacity-70"
        >
          <span className="truncate font-display text-[13px] font-semibold tracking-tight text-muted-foreground">
            Archived
          </span>
          <CaretRight
            size={11}
            className={cn(
              "shrink-0 text-muted-foreground transition-transform",
              open && "rotate-90",
            )}
          />
        </button>
        <span className="text-[11px] tabular-nums text-muted-foreground/60">
          {projects.length}
        </span>
      </div>
      {open && (
        <RowBox
          projects={projects}
          counts={counts}
          actions={actions}
          quiet
          emptyCopy="Nothing archived."
        />
      )}
    </section>
  );
}

export function ProjectFlatList({
  projects,
  counts,
  subjectLabel,
  onCreate,
  actions,
}: {
  projects: DbProject[];
  counts: Map<number, ProjectTaskCounts>;
  subjectLabel: string;
  onCreate: (name: string) => void;
  actions?: ProjectRowActions;
}) {
  return (
    <RowBox
      projects={projects}
      counts={counts}
      actions={actions}
      onCreate={onCreate}
      addLabel="New project"
      emptyCopy={`No projects for ${subjectLabel} yet — an assignment is usually the first one.`}
    />
  );
}
