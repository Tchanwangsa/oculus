import type { Lecture } from "@/lib/db";

/** The full-page player route; `t` titles the tab. */
export function lecturePagePath(
  lec: Pick<Lecture, "id" | "subject_id" | "title">,
): string {
  return `/subjects/${lec.subject_id}/lecture?id=${encodeURIComponent(lec.id)}&t=${encodeURIComponent(lec.title)}`;
}

/** The lecture id a `lecturePagePath` route names, or null for any other path. */
export function lecturePageId(path: string | null | undefined): string | null {
  const [pathname, search = ""] = (path ?? "").split("?");
  if (!/^\/subjects\/\d+\/lecture$/.test(pathname)) return null;
  return new URLSearchParams(search).get("id");
}
