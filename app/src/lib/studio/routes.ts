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

export const COMPONENTS_HREF = "/studio/components";

/** The Components page with one component selected. */
export function componentHref(name: string): string {
  return `${COMPONENTS_HREF}/${encodeURIComponent(name)}`;
}

export const THEMES_HREF = "/studio/themes";

/** The Themes page with one theme selected. */
export function themeHref(name: string): string {
  return `${THEMES_HREF}/${encodeURIComponent(name)}`;
}
