import { useCallback, useEffect, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { NewProjectButton } from "@/components/projects/NewProjectButton";
import { SectionHeader } from "@/components/projects/SectionHeader";
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
 *
 * **The landing view of the Tasks section**, whose other tab is `/tasks`
 * (`SectionHeader`). It has no `<h1>` of its own: the strip already names the
 * section, and a heading on the same rule would be a second title — the trade
 * `TasksPage` and `ProjectPage` both make. `NewProjectButton` sits in the
 * header's toolbar row, since a project is the thing this tab makes.
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
    <div className="flex h-full flex-col">
      <SectionHeader>
        <span className="flex-1" />
        {/* The way into a subject that has no projects yet: the groups below
            only draw once they have something in them, so a subject's first
            project has no heading to start it from. */}
        <NewProjectButton subjects={subjects} onCreate={create} />
      </SectionHeader>

      {/* The page's own body, under a full-width header: centred and capped,
          which the Tasks tab's board must not be. */}
      <div className="page-scroll min-h-0 flex-1">
        <div className="mx-auto max-w-3xl px-6 py-6">
          <ProjectGroups
            projects={active}
            counts={counts}
            subjects={subjects}
            onCreate={create}
            actions={actions}
          />

          {/* Only once there is something in it — the page's rule for every
              group but Personal, and an empty "Archived" heading would be the
              one thing on the page telling you about a state you are not in. */}
          {archived.length > 0 && (
            <>
              <div className="my-6 border-t border-border" />
              <ArchivedProjects projects={archived} counts={counts} actions={actions} />
            </>
          )}
        </div>
      </div>
    </div>
  );
}
