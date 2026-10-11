import { useMemo } from "react";
import { useProjectsStore } from "@/stores/planning/projectsStore";
import type { ProjectRowActions } from "./ProjectList";

/**
 * Project row writes bound to the store. Pages refresh via
 * `PROJECTS_UPDATED_EVENT`; failures are logged (no toasts).
 */
export function useProjectActions(): ProjectRowActions {
  const updateProject = useProjectsStore((s) => s.updateProject);
  const archiveProject = useProjectsStore((s) => s.archiveProject);
  const unarchiveProject = useProjectsStore((s) => s.unarchiveProject);
  const deleteProject = useProjectsStore((s) => s.deleteProject);

  return useMemo(
    () => ({
      onRename: (project, name) =>
        void updateProject(project.id, { name }).catch((e) =>
          console.error("rename project failed", e),
        ),
      onArchive: (project) =>
        void archiveProject(project.id).catch((e) =>
          console.error("archive project failed", e),
        ),
      onUnarchive: (project) =>
        void unarchiveProject(project.id).catch((e) =>
          console.error("unarchive project failed", e),
        ),
      onDelete: (project) =>
        void deleteProject(project.id).catch((e) =>
          console.error("delete project failed", e),
        ),
    }),
    [updateProject, archiveProject, unarchiveProject, deleteProject],
  );
}
