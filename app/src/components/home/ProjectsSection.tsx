import { useCallback, useState } from "react";
import { Link } from "react-router-dom";
import { projectHref } from "@/components/projects/projectHref";
import { displayCode } from "@/lib/format";
import {
  PROJECTS_UPDATED_EVENT,
  getProjects,
  getTaskCounts,
  type DbProject,
  type ProjectTaskCounts,
} from "@/lib/projects";
import { ROW, Section } from "./Section";
import { useHomeSection } from "./useHomeSection";

/**
 * Every project write fires this — a click on a board, and equally the chat
 * agent's own `oculus project` / `oculus task` run, which reaches the same
 * event through `useBackendEvents`. One door for both.
 *
 * Module-level so the reference is stable — see `useHomeSection`.
 */
const EVENTS = [PROJECTS_UPDATED_EVENT];

/** Four. The projects index is the list; this is the handful you are actually
 *  in, and a fifth row pushes Continue below the fold on a laptop. */
const MAX_ROWS = 4;

/**
 * The active projects, in the board's own order, with how far along each one
 * is.
 *
 * Counts come from one `getTaskCounts` over every id rather than a query per
 * row — the same reason the index and the subject tabs batch it. Archived
 * projects never appear: `getProjects` defaults to active, and a project you
 * have put away is not one to be reminded of on the first screen.
 */
export function ProjectsSection() {
  const [projects, setProjects] = useState<DbProject[]>([]);
  const [counts, setCounts] = useState<Map<number, ProjectTaskCounts>>(new Map());

  const reload = useCallback(() => {
    getProjects({})
      .then(async (all) => {
        const top = all.slice(0, MAX_ROWS);
        setProjects(top);
        setCounts(await getTaskCounts(top.map((p) => p.id)));
      })
      .catch((e) => {
        console.error(e);
        setProjects([]);
      });
  }, []);

  useHomeSection(reload, EVENTS);

  if (projects.length === 0) return null;

  return (
    <Section title="Projects">
      {projects.map((p) => {
        const c = counts.get(p.id);
        return (
          // `projectHref`, not a bare path: the tab strip titles a project tab
          // out of the `?n=` query and has no project list to look one up in.
          <Link key={p.id} to={projectHref(p)} className={ROW}>
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[12px] text-foreground">{p.name}</span>
              <span className="block truncate text-[11px] text-muted-foreground">
                {p.subject_code ? displayCode(p.subject_code) : "Personal"}
                {c && (
                  <>
                    {" · "}
                    <span className="tabular-nums">
                      {c.done}/{c.total}
                    </span>
                  </>
                )}
              </span>
            </span>
          </Link>
        );
      })}
    </Section>
  );
}
