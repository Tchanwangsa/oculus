import { useCallback, useEffect, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { NewProjectButton } from "@/components/projects/nav/NewProjectButton";
import { SectionHeader } from "@/components/projects/page/SectionHeader";
import { ArchivedProjects, ProjectGroups } from "@/components/projects/lists/ProjectList";
import { projectHref } from "@/components/projects/nav/projectHref";
import { useProjectActions } from "@/components/projects/lists/useProjectActions";
import { useSubjects } from "@/hooks/data/useSubjects";
import { PROJECTS_UPDATED_EVENT } from "@/lib/planning/projects";
import { useProjectsStore } from "@/stores/planning/projectsStore";
import { useWindowEvent } from "@/hooks/backend/useEvents";

/**
 * Every project, grouped by subject (subject-less under Personal), archived at
 * the bottom — the landing tab of the `Projects · Tasks` section
 * (`SectionHeader`). Reads `status: "all"` once and splits it, since the store
 * holds one list; the subject tab stays on the active default.
 */
export default function ProjectsIndexPage() {
  const { subjects } = useSubjects();
  const navigate = useNavigate();

  const projects = useProjectsStore((s) => s.projects);
  const counts = useProjectsStore((s) => s.counts);
  const loadProjects = useProjectsStore((s) => s.loadProjects);
  const createProject = useProjectsStore((s) => s.createProject);

  useEffect(() => {
    // Explicit: the store's query is shared and may be filtered to one subject.
    void loadProjects({ status: "all" });
  }, [loadProjects]);

  useWindowEvent(PROJECTS_UPDATED_EVENT, () => void loadProjects());

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
        // Straight into the new project.
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
        {/* Empty groups don't draw, so a subject's first project starts here. */}
        <NewProjectButton subjects={subjects} onCreate={create} />
      </SectionHeader>

      {/* `page-scroll` is `height: 100%`, so it goes inside the flex item. */}
      <div className="min-h-0 flex-1">
        <div className="page-scroll">
          <div className="mx-auto max-w-3xl px-6 py-6">
            <ProjectGroups
              projects={active}
              counts={counts}
              subjects={subjects}
              onCreate={create}
              actions={actions}
            />

            {archived.length > 0 && (
              <>
                <div className="my-6 border-t border-border" />
                <ArchivedProjects projects={archived} counts={counts} actions={actions} />
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
