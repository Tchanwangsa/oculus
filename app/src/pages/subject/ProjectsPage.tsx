import { useCallback, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { ProjectFlatList } from "@/components/projects/ProjectList";
import { projectHref } from "@/components/projects/projectHref";
import { useProjectActions } from "@/components/projects/useProjectActions";
import { useSubject } from "@/layouts/SubjectLayout";
import { displayCode } from "@/lib/format";
import { PROJECTS_UPDATED_EVENT } from "@/lib/projects";
import { useProjectsStore } from "@/stores/projectsStore";

/**
 * One subject's projects. The subject comes from the layout's outlet context —
 * a tab page never resolves it again.
 *
 * There is no archived section here, and that is the difference between this
 * page and the index: a subject's tab is where you work, and the index is where
 * you keep the record.
 */
export default function SubjectProjectsPage() {
  const subject = useSubject();
  const navigate = useNavigate();

  const projects = useProjectsStore((s) => s.projects);
  const counts = useProjectsStore((s) => s.counts);
  const loadProjects = useProjectsStore((s) => s.loadProjects);
  const createProject = useProjectsStore((s) => s.createProject);

  useEffect(() => {
    // `status` spelled out even though `"active"` is `getProjects`' default.
    // The store replaces its whole query rather than merging into it, so the
    // index asking for `"all"` cannot leak in here — but the two pages sit on
    // one store and both re-run *their own* query on every update, so which of
    // them this list belongs to should be readable without going and checking
    // what the default is.
    void loadProjects({ subjectId: subject.id, status: "active" });
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
