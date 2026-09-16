import { useMemo } from "react";
import { useProjectsStore } from "@/stores/projectsStore";
import type { ProjectRowActions } from "./ProjectList";

/**
 * Rename, archive, unarchive, delete — bound to the store once, for every
 * surface that offers them.
 *
 * Three pages reach for the same four writes (the index, a subject's Projects
 * tab, and a project's own header), and they were three identical blocks of
 * store calls before this: the kind of duplication that stays correct right up
 * until one of them grows a confirmation the others do not have. There is
 * nothing page-specific in any of them, because there is nothing to do after
 * the write — every one of these fires `PROJECTS_UPDATED_EVENT`, which each
 * page is already listening for, the same door an `oculus project` call from
 * the chat agent arrives through.
 *
 * Failures are logged rather than surfaced, which is this app's rule (no
 * toasts): a rename that did not take leaves the old name on screen at the
 * next re-read, which is the truthful outcome.
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
