/** The Studio page's paths (`app/src/routes.tsx`). The title rides in `?n=`
 *  because `tabInfo` titles a tab from its path alone. */
export const NEW_VIDEO_HREF = "/studio";
export const VIDEOS_HREF = "/studio/videos";

/** A video's detail, or with `thread` the conversation that made it. */
export function videoHref(slug: string, title?: string | null, thread = false): string {
  const base = `${VIDEOS_HREF}/${encodeURIComponent(slug)}${thread ? "/thread" : ""}`;
  const name = title?.trim();
  return name ? `${base}?n=${encodeURIComponent(name)}` : base;
}
