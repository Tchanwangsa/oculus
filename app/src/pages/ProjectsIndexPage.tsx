import { useCallback, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { ProjectGroups } from "@/components/projects/ProjectList";
import { projectHref } from "@/components/projects/projectHref";
import { useSubjects } from "@/hooks/useSubjects";
import { PROJECTS_UPDATED_EVENT } from "@/lib/projects";
import { useProjectsStore } from "@/stores/projectsStore";

/**
 * Every project you have on, grouped by the subject it belongs to, with the
 * subject-less ones under Personal.
 *
 * Archived projects are deliberately absent: `getProjects` defaults to active,
 * and an archived project is something you reach by its own link rather than
 * something the list keeps carrying.
 */
export default function ProjectsIndexPage() {
  const { subjects } = useSubjects();
  const navigate = useNavigate();

  const projects = useProjectsStore((s) => s.projects);
  const counts = useProjectsStore((s) => s.counts);
  const loadProjects = useProjectsStore((s) => s.loadProjects);
  const createProject = useProjectsStore((s) => s.createProject);

  useEffect(() => {
    // `{}` rather than no argument: the store's query is shared, and a subject
    // tab or a board may have left it filtered.
    void loadProjects({});
  }, [loadProjects]);

  useEffect(() => {
    const onUpdated = () => void loadProjects();
    window.addEventListener(PROJECTS_UPDATED_EVENT, onUpdated);
    return () => window.removeEventListener(PROJECTS_UPDATED_EVENT, onUpdated);
  }, [loadProjects]);

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

  return (
    <div className="page-scroll">
      <div className="mx-auto max-w-3xl px-6 py-6">
        <h1 className="text-[22px] font-semibold leading-none tracking-tight text-foreground">
          Projects
        </h1>
        <p className="mt-1.5 text-[13px] text-muted-foreground">
          Keep your tasks for upcoming deadlines planned and organised.
        </p>
        <div className="mt-5 mb-6 border-t border-border" />

        <ProjectGroups
          projects={projects}
          counts={counts}
          subjects={subjects}
          onCreate={create}
        />
      </div>
    </div>
  );
}
