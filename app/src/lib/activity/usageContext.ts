import { browseId } from "@/lib/browser";

// Each `usage_activity` ping carries the context of the page it came from, and
// Rust credits the tick's active seconds to it in `usage_context_hours`. The
// kinds are stored as written here; Home's Activity card folds them into fewer
// display groups.

export type UsageKind =
  | "lecture"
  | "file"
  | "document"
  | "course"
  | "chat"
  | "browser"
  | "planning"
  | "other";

export interface UsageContext {
  kind: UsageKind;
  /** The subject the page belongs to; null outside a subject. */
  subjectId: number | null;
}

/** A route's usage context, from the path alone (`?query` ignored). */
export function usageContext(path: string): UsageContext {
  const [pathname, search = ""] = path.split("?");
  if (browseId(pathname) != null) return { kind: "browser", subjectId: null };
  if (pathname.startsWith("/chat")) return { kind: "chat", subjectId: null };
  if (/^\/(projects|tasks|calendar)(\/|$)/.test(pathname)) return { kind: "planning", subjectId: null };

  const m = /^\/subjects\/(\d+)(?:\/([\w-]+))?/.exec(pathname);
  if (!m) return { kind: "other", subjectId: null };
  const subjectId = Number(m[1]);
  if (m[2] === "lecture") return { kind: "lecture", subjectId };
  if (m[2] === "file") {
    // The student's own notes live under `documents/`; everything else is a
    // downloaded or uploaded file.
    const rel = new URLSearchParams(search).get("path") ?? "";
    return { kind: rel.includes("/documents/") ? "document" : "file", subjectId };
  }
  if (m[2] === "projects") return { kind: "planning", subjectId };
  return { kind: "course", subjectId };
}

export function sameContext(a: UsageContext | null, b: UsageContext): boolean {
  return a !== null && a.kind === b.kind && a.subjectId === b.subjectId;
}
