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

/** Fired by every project write, UI or CLI (via `useBackendEvents`).
 *  Module-level for a stable reference — see `useHomeSection`. */
const EVENTS = [PROJECTS_UPDATED_EVENT];

const MAX_ROWS = 4;

/**
 * Active projects in board order with progress; counts batched in one
 * `getTaskCounts`. Archived projects never appear.
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
          // `projectHref`: the tab strip titles a project tab from `?n=`.
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
