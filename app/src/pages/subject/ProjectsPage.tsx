import { useCallback, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { ProjectFlatList } from "@/components/projects/ProjectList";
import { projectHref } from "@/components/projects/projectHref";
import { useSubject } from "@/layouts/SubjectLayout";
import { displayCode } from "@/lib/format";
import { PROJECTS_UPDATED_EVENT } from "@/lib/projects";
import { useProjectsStore } from "@/stores/projectsStore";

/**
 * One subject's projects. The subject comes from the layout's outlet context —
 * a tab page never resolves it again.
 */
export default function SubjectProjectsPage() {
  const subject = useSubject();
  const navigate = useNavigate();

  const projects = useProjectsStore((s) => s.projects);
  const counts = useProjectsStore((s) => s.counts);
  const loadProjects = useProjectsStore((s) => s.loadProjects);
  const createProject = useProjectsStore((s) => s.createProject);

  useEffect(() => {
    void loadProjects({ subjectId: subject.id });
  }, [loadProjects, subject.id]);

  useEffect(() => {
    const onUpdated = () => void loadProjects();
    window.addEventListener(PROJECTS_UPDATED_EVENT, onUpdated);
    return () => window.removeEventListener(PROJECTS_UPDATED_EVENT, onUpdated);
  }, [loadProjects]);

  const create = useCallback(
    (name: string) => {
      createProject({ name, subjectId: subject.id })
        .then((id) => navigate(projectHref({ id, name })))
        .catch((e) => console.error("create project failed", e));
    },
    [createProject, navigate, subject.id],
  );

  return (
    <div className="page-scroll">
      <div className="mx-auto max-w-5xl px-6 py-6">
        <ProjectFlatList
          projects={projects}
          counts={counts}
          subjectLabel={displayCode(subject.code)}
          onCreate={create}
        />
      </div>
    </div>
  );
}
