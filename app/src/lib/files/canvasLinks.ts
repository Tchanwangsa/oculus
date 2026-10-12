import { openLink } from "@/lib/shell/tabRouters";
import { currentInstitution } from "@/stores/sync/institutionStore";

/** Where a link out goes: a URL, or one read at the click, for a Canvas page
 *  whose host is the profile's (which may load after the first render). */
export type LinkOut = string | (() => Promise<string | null>);

/** Opens `link` beside the page; ⌘-click (`newTab`) as a tab of its own. */
export async function openLinkOut(link: LinkOut, newTab: boolean): Promise<void> {
  const url = typeof link === "string" ? link : await link();
  if (url) await openLink(url, newTab);
}

/** A Canvas file's page on `canvas`, the profile's `https://host`: under its
 *  course when the subject is known (a subject's id is its Canvas course id),
 *  else Canvas's course-less route. */
export function canvasFileUrl(canvas: string, fileId: string, courseId?: number): string {
  const id = encodeURIComponent(fileId);
  return courseId != null ? `${canvas}/courses/${courseId}/files/${id}` : `${canvas}/files/${id}`;
}

/** [`canvasFileUrl`] on the profile's Canvas, read when the link is opened. */
export function canvasFileLink(fileId: string, courseId?: number): LinkOut {
  return async () => {
    const { canvas } = await currentInstitution();
    return canvas ? canvasFileUrl(canvas, fileId, courseId) : null;
  };
}
