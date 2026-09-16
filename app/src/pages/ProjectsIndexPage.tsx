import { useCallback, useEffect, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { NewProjectButton } from "@/components/projects/NewProjectButton";
import { ArchivedProjects, ProjectGroups } from "@/components/projects/ProjectList";
import { projectHref } from "@/components/projects/projectHref";
import { useProjectActions } from "@/components/projects/useProjectActions";
import { useSubjects } from "@/hooks/useSubjects";
import { PROJECTS_UPDATED_EVENT } from "@/lib/projects";
import { useProjectsStore } from "@/stores/projectsStore";

/**
 * Every project you have on, grouped by the subject it belongs to, with the
 * subject-less ones under Personal, and the ones you have put away at the
 * bottom.
 *
 * The page asks for `status: "all"` and splits the result itself rather than
 * reading the list twice: the store holds one list and one set of counts, so a
 * second query would either overwrite the first or need a second store. Which
 * makes this the only page that sees archived rows — the subject tab is a
 * working view and deliberately stays on the default.
 */
export default function ProjectsIndexPage() {
  const { subjects } = useSubjects();
  const navigate = useNavigate();

  const projects = useProjectsStore((s) => s.projects);
  const counts = useProjectsStore((s) => s.counts);
  const loadProjects = useProjectsStore((s) => s.loadProjects);
  const createProject = useProjectsStore((s) => s.createProject);

  useEffect(() => {
    // Spelled out rather than left to the default: the store's query is shared,
    // and a subject tab or a board may have left it filtered to one subject.
    void loadProjects({ status: "all" });
  }, [loadProjects]);

  useEffect(() => {
    const onUpdated = () => void loadProjects();
    window.addEventListener(PROJECTS_UPDATED_EVENT, onUpdated);
    return () => window.removeEventListener(PROJECTS_UPDATED_EVENT, onUpdated);
  }, [loadProjects]);

  const { active, archived } = useMemo(
    () => ({
      active: projects.filter((p) => p.status !== "archived"),
      archived: projects.filter((p) => p.status === "archived"),
    }),
    [projects],
  );

  const create = useCallback(
    (subjectId: number | null, name: string) => {
      createProject({ name, subjectId })
        // Straight into the new project: a project is created to be filled in,
        // and the list has nothing more to tell you about an empty one.
        .then((id) => navigate(projectHref({ id, name })))
        .catch((e) => console.error("create project failed", e));
    },
    [createProject, navigate],
  );

  const actions = useProjectActions();

  return (
    <div className="page-scroll">
      <div className="mx-auto max-w-3xl px-6 py-6">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <h1 className="text-[22px] font-semibold leading-none tracking-tight text-foreground">
              Projects
            </h1>
            <p className="mt-1.5 text-[13px] text-muted-foreground">
              Keep your tasks for upcoming deadlines planned and organised.
            </p>
          </div>
          {/* The way into a subject that has no projects yet: the groups below
              only draw once they have something in them, so a subject's first
              project has no heading to start it from. */}
          <NewProjectButton subjects={subjects} onCreate={create} />
        </div>
        <div className="mt-5 mb-6 border-t border-border" />

        <ProjectGroups
          projects={active}
          counts={counts}
          subjects={subjects}
          onCreate={create}
          actions={actions}
        />

        {/* Only once there is something in it — the page's rule for every group
            but Personal, and an empty "Archived" heading would be the one thing
            on the page telling you about a state you are not in. */}
        {archived.length > 0 && (
          <>
            <div className="my-6 border-t border-border" />
            <ArchivedProjects projects={archived} counts={counts} actions={actions} />
          </>
        )}
      </div>
    </div>
  );
}
