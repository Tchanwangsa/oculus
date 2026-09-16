import {
  ArrowClockwise,
  ArrowsClockwise,
  BookOpen,
  CalendarBlank,
  Chat,
  GearSix,
  Globe,
  House,
  Kanban,
} from "@phosphor-icons/react";
import { browseId, hostOf, type BrowserTab } from "@/lib/browser";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode, humanizeSlug } from "@/lib/format";
import type { Subject } from "@/lib/db";

const SECTION_LABELS: Record<string, string> = {
  modules: "Modules",
  downloads: "Downloads",
  lectures: "Lectures",
  announcements: "Announcements",
  assignments: "Assignments",
  discussion: "Discussion",
};

export interface TabInfo {
  title: string;
  icon: React.ReactNode;
}

/**
 * What a route is called and what it looks like — the tab strip's naming,
 * shared so the sidebar's Recent list names a page exactly as its tab does.
 * Derived from the path on every render rather than stored with it, so a
 * renamed subject or a still-loading browser page follows on its own.
 */
export function tabInfo(
  path: string,
  subjects: Subject[],
  browserTabs: BrowserTab[],
  size = 13,
): TabInfo {
  const [pathname, search = ""] = path.split("?");
  // A browser tab is titled by its page, as a browser's is; the globe
  // spins while the page loads.
  const bid = browseId(pathname);
  if (bid != null) {
    const tab = browserTabs.find((t) => t.id === bid);
    return {
      title: tab?.title || hostOf(tab?.url ?? "") || "New tab",
      icon: tab?.loading ? (
        <ArrowClockwise size={size} className="animate-spin" />
      ) : (
        <Globe size={size} />
      ),
    };
  }
  // Exact, not a prefix: every other route starts with "/" too.
  if (pathname === "/") return { title: "Home", icon: <House size={size} /> };
  if (pathname.startsWith("/chat"))
    return { title: "Chat", icon: <Chat size={size} /> };
  if (pathname.startsWith("/calendar"))
    return { title: "Calendar", icon: <CalendarBlank size={size} /> };
  // A project is titled by itself, the way a lecture is: the name rides in the
  // query (`projectHref`), since this function has no project list to look one
  // up in and is called on every render of the strip.
  const proj = /^\/projects\/(\d+)/.exec(pathname);
  if (proj) {
    return {
      title: new URLSearchParams(search).get("n") || "Project",
      icon: <Kanban size={size} />,
    };
  }
  if (pathname.startsWith("/projects"))
    return { title: "Projects", icon: <Kanban size={size} /> };
  if (pathname.startsWith("/sync"))
    return { title: "Sync", icon: <ArrowsClockwise size={size} /> };
  if (pathname.startsWith("/settings"))
    return { title: "Settings", icon: <GearSix size={size} /> };
  const m = /^\/subjects\/(\d+)(?:\/([\w-]+))?/.exec(pathname);
  if (m) {
    const subject = subjects.find((s) => String(s.id) === m[1]);
    // Anything inside a subject carries the subject's identity glyph.
    const icon = subject ? (
      <SubjectIcon code={subject.code} size={size} />
    ) : (
      <BookOpen size={size} />
    );
    // Full-page documents are titled by themselves, like Notion pages.
    if (m[2] === "file") {
      const rel = new URLSearchParams(search).get("path");
      const base = rel?.split("/").pop();
      if (base)
        return { title: humanizeSlug(base.replace(/\.pdf$/i, "")), icon };
    }
    if (m[2] === "lecture") {
      return { title: new URLSearchParams(search).get("t") ?? "Lecture", icon };
    }
    const code = subject ? displayCode(subject.code) : "Subject";
    const section = m[2] ? SECTION_LABELS[m[2]] : null;
    return { title: section ? `${code} · ${section}` : code, icon };
  }
  if (pathname.startsWith("/subjects"))
    return { title: "Subjects", icon: <BookOpen size={size} /> };
  return { title: "Oculus", icon: null };
}
