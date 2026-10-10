/** Fired after any write here so open boards re-read — a window event,
 *  since these writes start in the frontend. */
export const PROJECTS_UPDATED_EVENT = "oculus:projects-updated";

export function notifyProjectsUpdated(): void {
  window.dispatchEvent(new CustomEvent(PROJECTS_UPDATED_EVENT));
}
