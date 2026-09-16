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

const COLLAPSED_KEY = "oculus-projects-groups-collapsed";

/** Which groups are folded, by key. The *collapsed* ones are stored, so a
 *  subject that gets its first project arrives open without being listed
 *  anywhere first — `ThreadList`'s rule. */
function loadCollapsed(): Set<string> {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? "[]");
    return new Set(Array.isArray(raw) ? raw.filter((k): k is string => typeof k === "string") : []);
  } catch {
    return new Set();
  }
}

/**
 * What a row's overflow menu does, bound to the row it was opened on.
 *
 * One object rather than four props at every hop, because it is threaded three
 * levels down — row, box, list — and none of the levels in between has an
 * opinion about any of them. It is **optional** the whole way: a list that
 * passes nothing draws no menu, which is a truer answer than a list that has
 * to invent four handlers to say it has no actions to offer.
 *
 * Every callback takes the project, so a page can act on a row without the row
 * closing over which page it is in.
 */
export interface ProjectRowActions {
  /** The new name, trimmed and known to differ — {@link ProjectMenu} swallows
   *  the no-op. */
  onRename: (project: DbProject, name: string) => void;
  onArchive: (project: DbProject) => void;
  onUnarchive: (project: DbProject) => void;
  onDelete: (project: DbProject) => void;
}

/**
 * Projects under the subject they are scoped to, in the order the list came
 * back in, with the subject-less ones under "Personal".
 *
 * A subject that has since been deleted falls back into Personal rather than
 * vanishing with its heading, which is what `ThreadList.group` does with a
 * thread whose subject is gone.
 */
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

/**
 * One project as a row: what it is called, what it is about, how far along it
 * is, when it is due.
 *
 * The link covers the row's *contents* and the menu sits beside it as a
 * sibling, not inside it. An anchor wrapping the whole row is the simpler
 * markup and was the earlier shape, but a button nested in a link navigates
 * when you click it — and there is no cancelling that from the button, since
 * the anchor is the thing the browser acts on. So the hover surface moved up
 * to the wrapper, which is what keeps the row lighting as one row while the
 * pointer is on the menu.
 */
export function ProjectRow({
  project,
  counts,
  actions,
  quiet = false,
}: {
  project: DbProject;
  counts: ProjectTaskCounts | undefined;
  actions?: ProjectRowActions;
  /** Drawn as something you have put away: the name steps back and the
   *  progress meter gives up its bar. An archived project is a record, not
   *  work in flight, and a half-filled bar on one reads as a project that
   *  stalled. */
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
            /* Faded rather than `hidden`, for two reasons: a display-none
               button cannot be tabbed to, and a button that only exists on
               hover would re-lay the row out as the pointer crosses it. It
               stays up while its own popover is open, or the menu would be
               anchored to something invisible. */
            className="opacity-0 transition-opacity group-hover/row:opacity-100 focus-visible:opacity-100 data-[state=open]:opacity-100"
          />
        </span>
      )}
    </div>
  );
}

/** The bordered column a group's rows sit in — shared so the index and the
 *  subject tab draw the same list. */
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
  /** Omitted where the box is not somewhere new work starts — the archived
   *  list is a record, and a composer in it would create an active project
   *  under an "Archived" heading. */
  onCreate?: (name: string) => void;
  addLabel?: string;
  emptyCopy: string;
  actions?: ProjectRowActions;
  quiet?: boolean;
  /** Open the composer as a focused field rather than a button — what the
   *  group heading's `+` does from outside the box. The `key` below remounts
   *  it so arming works a second time. */
  armed?: boolean;
}) {
  return (
    <div className="divide-y divide-border-subtle overflow-hidden rounded-lg border border-border">
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
    </div>
  );
}

/** Every project, grouped by subject — the index. */
export function ProjectGroups({
  projects,
  counts,
  subjects,
  onCreate,
  actions,
}: {
  projects: DbProject[];
  /** Finished/total per project id, from the store — one grouped read for the
   *  whole list rather than a query per row. */
  counts: Map<number, ProjectTaskCounts>;
  subjects: Subject[];
  /** `null` is the Personal group. */
  onCreate: (subjectId: number | null, name: string) => void;
  actions?: ProjectRowActions;
}) {
  const [folded, setFolded] = useState<Set<string>>(loadCollapsed);
  /** The group whose composer the heading's `+` has just opened. */
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

  // The Personal group is always offered, even empty: it is where a project
  // that belongs to no subject goes, and it cannot be discovered if it only
  // appears once one exists.
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
                {/* Not an <h2>: a heading element cannot live inside a button,
                    so it borrows the base heading rule's face instead. */}
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
                /* A project started from a folded group would be started out
                   of sight, so the group opens with the composer. */
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

/**
 * Whether the archived section is open, stored the *other* way round from
 * {@link COLLAPSED_KEY}.
 *
 * That key stores which groups are folded so that anything new — a subject's
 * first project — arrives open without having to be listed somewhere first.
 * Here the wanted default is the opposite: archived is closed until you go
 * looking, so what is worth storing is the one state that differs from the
 * default. Absent means shut, which is what a first run should be.
 */
const ARCHIVED_OPEN_KEY = "oculus-projects-archived-open";

/**
 * The projects you have put away, under everything else.
 *
 * Drawn by its caller only when it has rows, on the index's own rule that a
 * heading appears once there is something under it — an always-present
 * "Archived (0)" would be the one empty heading on a page arranged to have
 * none. Personal is the deliberate exception there, because it is the only way
 * to discover where a subject-less project goes; archived has a way in already,
 * which is having archived something.
 */
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

/** One subject's projects, ungrouped — the per-subject tab. */
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
