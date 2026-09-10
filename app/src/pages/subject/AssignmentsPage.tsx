import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowSquareOut, PencilLine, Rocket } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Skeleton } from "@/components/ui/skeleton";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { useModuleTocs } from "@/hooks/useModuleTocs";
import { useSubject } from "@/layouts/SubjectLayout";
import { openFileSmart } from "@/lib/openFile";
import { FileRecency } from "@/components/files/FileRecency";
import { humanizeSlug } from "@/lib/format";
import type { DbFile } from "@/lib/db";

interface TaskDoc {
  file: DbFile;
  title: string;
  kind: "quiz" | "assignment";
  /** Parsed from the doc's `**Due:**` line; null when Canvas set no due date. */
  due: Date | null;
  /** `**Available until:**` — when Canvas stops accepting submissions. */
  lock: Date | null;
  /** `**Status:** submitted|graded` — the user has handed it in. */
  submitted: boolean;
  points: string | null;
}

/** `**<label>:** 2026-09-12 13:59 UTC` (written by sync.rs) → a local Date. */
function parseTs(md: string, label: string): Date | null {
  const m = new RegExp(
    `^\\*\\*${label}:\\*\\* (\\d{4}-\\d{2}-\\d{2}) (\\d{2}:\\d{2}) UTC\\s*$`,
    "m",
  ).exec(md);
  if (!m) return null;
  const d = new Date(`${m[1]}T${m[2]}:00Z`);
  return Number.isNaN(d.getTime()) ? null : d;
}

// ── Grouping ─────────────────────────────────────────────────────────────────

const WEEK_MS = 7 * 24 * 60 * 60 * 1000;

const GROUPS = ["due-soon", "overdue", "upcoming", "closed", "done"] as const;
type Group = (typeof GROUPS)[number];

const GROUP_LABELS: Record<Group, string> = {
  "due-soon": "Due soon",
  overdue: "Overdue",
  upcoming: "Upcoming",
  closed: "Closed",
  done: "Done",
};

function groupOf(t: TaskDoc, now: number): Group {
  if (t.submitted) return "done";
  if (t.lock != null && t.lock.getTime() < now) return "closed";
  if (t.due != null && t.due.getTime() < now) return "overdue";
  if (t.due != null && t.due.getTime() - now < WEEK_MS) return "due-soon";
  return "upcoming";
}

/**
 * Loads every scraped assignment/quiz document and lifts its header metadata
 * (title, due date, points) into list rows. `null` while loading.
 */
function useTaskDocs(files: DbFile[], loading: boolean): TaskDoc[] | null {
  const [docs, setDocs] = useState<TaskDoc[] | null>(null);

  useEffect(() => {
    if (files.length === 0) {
      if (!loading) setDocs([]);
      return;
    }
    let cancelled = false;
    Promise.all(
      files.map(async (file): Promise<TaskDoc> => {
        const md = await invoke<string>("read_course_file", {
          relativePath: file.relative_path,
        }).catch(() => "");
        return {
          file,
          title: /^# (.+)$/m.exec(md)?.[1] ?? humanizeSlug(file.filename),
          kind: file.category === "quiz" ? "quiz" : "assignment",
          due: parseTs(md, "Due"),
          lock: parseTs(md, "Available until"),
          submitted: /^\*\*Status:\*\* (submitted|graded)\s*$/m.test(md),
          points: /^\*\*Points:\*\* (.+?)\s*$/m.exec(md)?.[1] ?? null,
        };
      }),
    ).then((loaded) => {
      if (!cancelled) setDocs(loaded);
    });
    return () => {
      cancelled = true;
    };
  }, [files, loading]);

  return docs;
}

function fmtDue(d: Date): string {
  return d.toLocaleString("en-AU", {
    weekday: "short",
    day: "numeric",
    month: "short",
    hour: "numeric",
    minute: "2-digit",
  });
}

/**
 * Quizzes and assignments, from the documents the scraper writes to
 * `assignments/` and `quizzes/` — due soonest first, rows open in the peek.
 * Libraries synced before those existed fall back to the module-TOC listing,
 * whose rows can only link out to Canvas.
 */
export default function SubjectAssignmentsPage() {
  const subject = useSubject();
  const { byCategory, loading } = useSubjectFiles(subject.id);

  const taskFiles = useMemo(
    () => [...byCategory.assignment, ...byCategory.quiz],
    [byCategory.assignment, byCategory.quiz],
  );
  const docs = useTaskDocs(taskFiles, loading);

  const grouped = useMemo(() => {
    if (!docs) return null;
    // Dated tasks by deadline; the undated tail keeps a stable name order.
    const sorted = [...docs].sort((a, b) => {
      if (a.due && b.due) return a.due.getTime() - b.due.getTime();
      if (a.due !== b.due) return a.due ? -1 : 1;
      return a.title.localeCompare(b.title);
    });
    const now = Date.now();
    const byGroup = new Map<Group, TaskDoc[]>();
    for (const t of sorted) {
      const g = groupOf(t, now);
      byGroup.set(g, [...(byGroup.get(g) ?? []), t]);
    }
    return byGroup;
  }, [docs]);

  if (grouped == null) {
    return (
      <div className="page-scroll">
        <div className="mx-auto max-w-5xl px-6 py-6 space-y-2">
          {Array.from({ length: 4 }).map((_, i) => (
            <Skeleton key={i} className="h-10 w-full" />
          ))}
        </div>
      </div>
    );
  }

  if ([...grouped.values()].every((g) => g.length === 0)) {
    return <TocFallback />;
  }

  return (
    <div className="page-scroll">
      <div className="mx-auto max-w-5xl px-6 py-5 space-y-5">
        {GROUPS.map((group) => {
          const tasks = grouped.get(group);
          if (!tasks || tasks.length === 0) return null;
          return (
            <section key={group}>
              <h2 className="mb-1.5 px-0.5 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
                {GROUP_LABELS[group]}
              </h2>
              <div className="rounded-lg border border-border divide-y divide-border-subtle overflow-hidden">
                {tasks.map((t) => (
                  <TaskRow key={t.file.id} task={t} muted={group === "closed" || group === "done"} />
                ))}
              </div>
            </section>
          );
        })}
      </div>
    </div>
  );
}

function TaskRow({ task: t, muted }: { task: TaskDoc; muted: boolean }) {
  const Icon = t.kind === "quiz" ? Rocket : PencilLine;
  return (
    <button
      onClick={() => openFileSmart(t.file)}
      className={cn(
        "w-full flex items-center gap-3 px-3 py-2.5 text-left hover:bg-surface transition-colors",
        muted && "opacity-70",
      )}
    >
      <Icon size={13} className="shrink-0 opacity-60" />
      <span className="min-w-0 flex-1">
        <span className="block text-[12px] text-foreground truncate">
          {t.title}
        </span>
        <span className="block text-[11px] text-muted-foreground truncate">
          {t.due ? `Due ${fmtDue(t.due)}` : "No due date"}
          {t.points != null && ` · ${t.points} pts`}
        </span>
      </span>
      <FileRecency file={t.file} />
    </button>
  );
}

// ── Pre-resync fallback ──────────────────────────────────────────────────────

interface TocTask {
  title: string;
  kind: "quiz" | "assignment";
  moduleTitle: string;
  /** Canvas URL — nothing local to open, so rows go to the browser. */
  url: string | null;
}

/** The old module-TOC listing, shown until a sync writes real documents. */
function TocFallback() {
  const subject = useSubject();
  const { byCategory, loading } = useSubjectFiles(subject.id);
  const modules = useModuleTocs(byCategory.module, loading);

  const tasks = useMemo<TocTask[]>(() => {
    if (!modules) return [];
    const out: TocTask[] = [];
    for (const mod of modules) {
      for (const section of mod.sections) {
        for (const item of section.items) {
          if (item.kind === "quiz" || item.kind === "assignment") {
            out.push({
              title: item.title,
              kind: item.kind,
              moduleTitle: mod.title,
              url: item.href && /^https?:/i.test(item.href) ? item.href : null,
            });
          }
        }
      }
    }
    return out;
  }, [modules]);

  if (tasks.length === 0) {
    return (
      <div className="h-full flex flex-col items-center justify-center gap-2">
        <PencilLine size={24} className="text-muted-foreground/40" />
        <p className="text-sm text-muted-foreground">
          No quizzes or assignments scraped yet — run a sync.
        </p>
      </div>
    );
  }

  return (
    <div className="page-scroll">
      <div className="mx-auto max-w-5xl px-6 py-5">
        <div className="rounded-lg border border-border divide-y divide-border-subtle overflow-hidden">
          {tasks.map((t, i) => {
            const Icon = t.kind === "quiz" ? Rocket : PencilLine;
            const inner = (
              <>
                <Icon size={13} className="shrink-0 opacity-60" />
                <span className="min-w-0 flex-1">
                  <span className="block text-[12px] text-foreground truncate">
                    {t.title}
                  </span>
                  <span className="block text-[11px] text-muted-foreground truncate">
                    {t.moduleTitle}
                  </span>
                </span>
                <span className="shrink-0 text-[10px] uppercase tracking-wide text-muted-foreground">
                  {t.kind}
                </span>
                {t.url && (
                  <ArrowSquareOut size={12} className="shrink-0 opacity-40" />
                )}
              </>
            );
            const rowClass =
              "w-full flex items-center gap-3 px-3 py-2.5 text-left transition-colors";
            return t.url ? (
              <a
                key={i}
                href={t.url}
                target="_blank"
                rel="noreferrer"
                className={`${rowClass} hover:bg-surface`}
              >
                {inner}
              </a>
            ) : (
              <div key={i} className={`${rowClass} cursor-default`}>
                {inner}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
