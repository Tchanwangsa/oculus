/** The Canvas instance the app signs in to (`auth/mod.rs` checks the same host). */
export const CANVAS_ORIGIN = "https://canvas.lms.unimelb.edu.au";

/** A Canvas file's page: under its course when the subject is known (a
 *  subject's id is its Canvas course id), else Canvas's course-less route. */
export function canvasFileUrl(fileId: string, courseId?: number): string {
  const id = encodeURIComponent(fileId);
  return courseId != null ? `${CANVAS_ORIGIN}/courses/${courseId}/files/${id}` : `${CANVAS_ORIGIN}/files/${id}`;
}
