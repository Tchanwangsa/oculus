import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { ROW, Section } from "@/components/home/Section";
import { displayCode, sqliteUtcToMs } from "@/lib/format";
import type { DbProject, DbProjectTask, UpdateProjectInput } from "@/lib/projects";
import { DateTimeField } from "./DateTimeField";
import { EventLink } from "./EventLink";
import { TagEditor } from "./TagEditor";
import { DueChip, ProgressMeter, TaskGlyph } from "./TaskMarks";
import { taskHref } from "./taskHref";
import { boardProgress, columnOf, type TaskNode } from "./taskTree";

/**
 * What a project *is*, before what is left to do about it: a description, the
 * handful of facts worth pinning to it, and the work that is next.
 *
 * Short on purpose. Everything here could be a board column or a table cell
 * instead, and the reason it is not is that this is the page you open when you
 * have forgotten what the project was for — a fourth view of the same task
 * rows would answer a question the Tasks tab already answers.
 */

// ── Shared furniture ─────────────────────────────────────────────────────────

/**
 * The page's headings, which are `Section`'s own
 * (`app/src/components/home/Section.tsx`) down to the class list.
 *
 * Upcoming tasks below literally *is* a `Section`, so About and Properties —
 * which are not row lists and so have no box to put themselves in — borrow its
 * label rather than inventing a second heading weight for the same column.
 */
function Heading({ children }: { children: ReactNode }) {
  return (
    <h2 className="mb-2 px-0.5 text-[13px] font-semibold text-foreground">{children}</h2>
  );
}

/**
 * Notion's property grammar, the same row `app/src/pages/TaskPage.tsx` draws
 * one level down: the label is furniture on the left, the value is the control
 * on the right.
 *
 * `items-start` rather than that page's `items-center`, because two of these
 * values are as tall as their contents — a tag list wraps, and a resolved
 * event is two lines.
 */
function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex min-h-8 items-start gap-3">
      <span className="w-24 shrink-0 pt-1.5 text-[11px] text-muted-foreground">{label}</span>
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-2 py-0.5">{children}</div>
    </div>
  );
}

// ── The page ─────────────────────────────────────────────────────────────────

export function ProjectOverview({
  project,
  nodes,
  onPatch,
}: {
  project: DbProject;
  nodes: TaskNode[];
  /** Every write on this page, funnelled back to the one caller that holds the
   *  store — the same shape `ProjectBoard` has with `onMove`. */
  onPatch: (patch: UpdateProjectInput) => void;
}) {
  const progress = useMemo(() => boardProgress(nodes), [nodes]);

  return (
    <div className="page-scroll">
      <div className="mx-auto flex max-w-3xl flex-col gap-8 px-6 py-6">
        <section>
          <Heading>About</Heading>
          <Brief project={project} onSave={(brief) => onPatch({ brief })} />
        </section>

        <section>
          <Heading>Properties</Heading>
          <div className="flex flex-col gap-0.5">
            <Row label="Subject">
              {/* Read-only: moving a project between subjects re-scopes every
                  task's place on the calendar, and there is nowhere on a page
                  this quiet to say so. */}
              <span className="flex min-w-0 items-center gap-1.5 text-xs text-foreground">
                {project.subject_code ? (
                  <>
                    <SubjectIcon code={project.subject_code} size={13} />
                    {displayCode(project.subject_code)}
                  </>
                ) : (
                  "Personal"
                )}
              </span>
            </Row>

            <Row label="Status">
              <Badge variant="secondary" className="text-[11px]">
                {project.status === "archived" ? "Archived" : "Active"}
              </Badge>
            </Row>

            <Row label="Starts">
              <DateTimeField
                value={project.starts_at}
                defaultTime="start"
                onCommit={(startsAt) => onPatch({ startsAt })}
              />
            </Row>

            <Row label="Due">
              <DateTimeField
                value={project.due_at}
                defaultTime="end"
                onCommit={(dueAt) => onPatch({ dueAt })}
              />
            </Row>

            <Row label="Tags">
              <TagEditor tags={project.tags} onChange={(tags) => onPatch({ tags })} />
            </Row>

            <Row label="Event">
              <EventLink project={project} onPick={(eventId) => onPatch({ eventId })} />
            </Row>

            <Row label="Progress">
              {progress.total === 0 ? (
                // A bar at zero is what a stalled project looks like, and one
                // you have not broken down yet is not stalled — the same call
                // `ProjectProgress` makes on a list row.
                <span className="text-[11px] text-muted-foreground/60">Nothing planned yet</span>
              ) : (
                <ProgressMeter done={progress.done} total={progress.total} />
              )}
            </Row>
          </div>
        </section>

        <UpcomingTasks project={project} nodes={nodes} />
      </div>
    </div>
  );
}

/**
 * The description.
 *
 * Plain text, deliberately — the same call `TaskPage`'s body makes: a brief is
 * a paragraph saying what the assignment actually asks for, and wiring the
 * markdown renderer in would mean a read mode and an edit mode for something
 * you write three lines into. `whitespace-pre-wrap` is the whole renderer.
 *
 * The draft follows the row only when the row changes: every write in the app
 * fires `PROJECTS_UPDATED_EVENT`, and a draft that re-synced on each one would
 * overwrite what is being typed.
 */
function Brief({
  project,
  onSave,
}: {
  project: DbProject;
  onSave: (brief: string | null) => void;
}) {
  const [draft, setDraft] = useState(project.brief ?? "");
  useEffect(() => setDraft(project.brief ?? ""), [project.id, project.brief]);

  const save = () => {
    const brief = draft.trim();
    if (brief === (project.brief ?? "")) return;
    onSave(brief || null);
  };

  return (
    <textarea
      value={draft}
      placeholder="What is this project?"
      onChange={(e) => setDraft(e.target.value)}
      onBlur={save}
      onKeyDown={(e) => {
        // ⌘↵ saves without leaving the field — the composer's gesture, and the
        // only way to save a brief you are not finished with.
        if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
          e.preventDefault();
          save();
        }
      }}
      className={cn(
        "field-sizing-content min-h-20 w-full resize-none whitespace-pre-wrap rounded-lg border border-transparent",
        "bg-transparent px-2 py-1.5 text-[13px] leading-relaxed text-foreground outline-none transition-colors",
        "hover:border-border-subtle focus:border-brand/40 focus:bg-card",
        "placeholder:text-muted-foreground/60",
      )}
    />
  );
}

// ── Upcoming tasks ───────────────────────────────────────────────────────────

/** Five. This is the "what is next" glance, not a second table — past five
 *  rows you are reading the board, and the Tasks tab is one click away. */
const MAX_UPCOMING = 5;

interface Upcoming {
  task: DbProjectTask;
  /** The parent's title on a subtask. A bare "Draft the intro" on its own is
   *  not something you can act on, and the board always draws a subtask under
   *  the card it belongs to — here there is no card to sit under. */
  parentTitle: string | null;
  due: number;
}

/**
 * The project's own unfinished, dated work, soonest first — **subtasks
 * included**, because they are work, and a breakdown whose parents alone were
 * listed would skip exactly the rows that carry the real dates.
 *
 * Overdue sorts to the top for free: it is the same ascending order, and
 * `DueChip` already reddens anything behind us.
 */
function upcomingTasks(nodes: TaskNode[]): Upcoming[] {
  const out: Upcoming[] = [];
  const take = (task: DbProjectTask, parentTitle: string | null) => {
    if (task.done_at != null) return;
    const due = sqliteUtcToMs(task.due_at);
    if (due == null) return;
    out.push({ task, parentTitle, due });
  };
  for (const node of nodes) {
    take(node.task, null);
    for (const child of node.children) take(child, node.task.title);
  }
  return out.sort((a, b) => a.due - b.due).slice(0, MAX_UPCOMING);
}

/**
 * Two empty states, not one.
 *
 * A project you have not broken down yet and a project whose tasks carry no
 * dates are different problems with different next moves, and one shared
 * "nothing here" sentence would name neither. Neither draws the bordered box:
 * an empty list with a border around it reads as a list that failed to load.
 */
function UpcomingTasks({ project, nodes }: { project: DbProject; nodes: TaskNode[] }) {
  const rows = useMemo(() => upcomingTasks(nodes), [nodes]);
  const anyTasks = nodes.length > 0;

  if (rows.length === 0) {
    return (
      <section>
        <Heading>Upcoming tasks</Heading>
        <p className="px-0.5 text-xs text-muted-foreground">
          {anyTasks
            ? "Nothing here has a date on it yet. A task needs a due date before it can be next — and before the calendar can draw it."
            : "No tasks yet. Break this down on the Tasks tab and what is next will show up here."}
        </p>
      </section>
    );
  }

  return (
    <Section title="Upcoming tasks">
      {rows.map(({ task, parentTitle }) => (
        <Link key={task.id} to={taskHref(project.id, task)} className={ROW}>
          {/* The kind of the column it sits in, not the column's name, which is
              the user's to change — the rule `moveTask` decides "done" by. */}
          <TaskGlyph kind={columnOf(project, task.column_id)?.kind ?? null} />
          <span className="min-w-0 flex-1">
            <span className="block truncate text-[12px] text-foreground">{task.title}</span>
            {parentTitle && (
              <span className="block truncate text-[11px] text-muted-foreground">
                {parentTitle}
              </span>
            )}
          </span>
          <DueChip dueAt={task.due_at} />
        </Link>
      ))}
    </Section>
  );
}
