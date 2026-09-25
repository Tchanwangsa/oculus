import { useCallback, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { ProjectFlatList } from "@/components/projects/ProjectList";
import { projectHref } from "@/components/projects/projectHref";
import { useProjectActions } from "@/components/projects/useProjectActions";
import { useSubject } from "@/layouts/SubjectLayout";
import { displayCode } from "@/lib/format";
import { PROJECTS_UPDATED_EVENT } from "@/lib/projects";
import { useProjectsStore } from "@/stores/projectsStore";
import { useWindowEvent } from "@/hooks/useEvents";

/** One subject's projects. No archived section: that belongs to the index. */
export default function SubjectProjectsPage() {
  const subject = useSubject();
  const navigate = useNavigate();

  const projects = useProjectsStore((s) => s.projects);
  const counts = useProjectsStore((s) => s.counts);
  const loadProjects = useProjectsStore((s) => s.loadProjects);
  const createProject = useProjectsStore((s) => s.createProject);

  useEffect(() => {
    // Spelled out though it is the default: the store is shared with the index.
    void loadProjects({ subjectId: subject.id, status: "active" });
  }, [loadProjects, subject.id]);

  useWindowEvent(PROJECTS_UPDATED_EVENT, () => void loadProjects());

  const create = useCallback(
    (name: string) => {
      createProject({ name, subjectId: subject.id })
        .then((id) => navigate(projectHref({ id, name })))
        .catch((e) => console.error("create project failed", e));
    },
    [createProject, navigate, subject.id],
  );

  const actions = useProjectActions();

  return (
    <div className="page-scroll">
      <div className="mx-auto max-w-5xl px-6 py-6">
        <ProjectFlatList
          projects={projects}
          counts={counts}
          subjectLabel={displayCode(subject.code)}
          onCreate={create}
          actions={actions}
        />
      </div>
    </div>
  );
}
