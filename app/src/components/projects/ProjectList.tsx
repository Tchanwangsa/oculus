import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { CaretRight, Plus } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode, displayName } from "@/lib/format";
import type { Subject } from "@/lib/db";
import type { DbProject, ProjectTaskCounts } from "@/lib/projects";
import { InlineAdd } from "./InlineAdd";
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

/** One project as a row: what it is called, what it is about, how far along it
 *  is, when it is due. */
export function ProjectRow({
  project,
  counts,
}: {
  project: DbProject;
  counts: ProjectTaskCounts | undefined;
}) {
  return (
    <Link
      to={projectHref(project)}
      className="flex items-center gap-3 px-3 py-2.5 transition-colors hover:bg-surface"
    >
      <span className="min-w-0 flex-1">
        <span className="flex items-center gap-1.5">
          <span className="truncate text-[12px] text-foreground">{project.name}</span>
          <AgentMark source={project.source} />
        </span>
        {project.brief && (
          <span className="mt-0.5 block truncate text-[11px] text-muted-foreground">
            {project.brief}
          </span>
        )}
      </span>
      <ProjectProgress counts={counts} />
      <DueChip dueAt={project.due_at} />
      <CaretRight size={11} className="shrink-0 text-muted-foreground/40" />
    </Link>
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
  armed = false,
}: {
  projects: DbProject[];
  counts: Map<number, ProjectTaskCounts>;
  onCreate: (name: string) => void;
  addLabel: string;
  emptyCopy: string;
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
        <ProjectRow key={p.id} project={p} counts={counts.get(p.id)} />
      ))}
      <div className="px-1.5 py-1">
        <InlineAdd
          key={armed ? "armed" : "idle"}
          defaultEditing={armed}
          label={addLabel}
          placeholder="Project name"
          onAdd={onCreate}
        />
      </div>
    </div>
  );
}

/** Every project, grouped by subject — the index. */
export function ProjectGroups({
  projects,
  counts,
  subjects,
  onCreate,
}: {
  projects: DbProject[];
  /** Finished/total per project id, from the store — one grouped read for the
   *  whole list rather than a query per row. */
  counts: Map<number, ProjectTaskCounts>;
  subjects: Subject[];
  /** `null` is the Personal group. */
  onCreate: (subjectId: number | null, name: string) => void;
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

/** One subject's projects, ungrouped — the per-subject tab. */
export function ProjectFlatList({
  projects,
  counts,
  subjectLabel,
  onCreate,
}: {
  projects: DbProject[];
  counts: Map<number, ProjectTaskCounts>;
  subjectLabel: string;
  onCreate: (name: string) => void;
}) {
  return (
    <RowBox
      projects={projects}
      counts={counts}
      onCreate={onCreate}
      addLabel="New project"
      emptyCopy={`No projects for ${subjectLabel} yet — an assignment is usually the first one.`}
    />
  );
}
