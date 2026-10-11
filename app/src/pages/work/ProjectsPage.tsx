import { SubjectPage } from "@/components/subjects/SubjectPage";
import { useCallback, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { ProjectFlatList } from "@/components/projects/lists/ProjectList";
import { projectHref } from "@/components/projects/nav/projectHref";
import { useProjectActions } from "@/components/projects/lists/useProjectActions";
import { useSubject } from "@/layouts/SubjectLayout";
import { displayCode } from "@/lib/format/format";
import { PROJECTS_UPDATED_EVENT } from "@/lib/planning/projects";
import { useProjectsStore } from "@/stores/planning/projectsStore";
import { useWindowEvent } from "@/hooks/backend/useEvents";

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
    <SubjectPage className="py-6">
      <ProjectFlatList
        projects={projects}
        counts={counts}
        subjectLabel={displayCode(subject.code)}
        onCreate={create}
        actions={actions}
      />
    </SubjectPage>
  );
}
