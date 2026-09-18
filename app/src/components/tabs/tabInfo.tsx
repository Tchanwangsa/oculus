import {
  ArrowClockwise,
  ArrowsClockwise,
  BookOpen,
  CalendarBlank,
  Chat,
  CheckSquare,
  FileDashed,
  GearSix,
  Globe,
  House,
  Kanban,
  ListChecks,
} from "@phosphor-icons/react";
import { browseId, hostOf, type BrowserTab } from "@/lib/browser";
import { faviconFor } from "@/hooks/useBrowserTabs";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode, humanizeSlug } from "@/lib/format";
import type { Subject } from "@/lib/db";

const SECTION_LABELS: Record<string, string> = {
  modules: "Modules",
  downloads: "Downloads",
  uploads: "Uploads",
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
  favicons: Record<string, string> = {},
): TabInfo {
  const [pathname, search = ""] = path.split("?");
  // A browser tab is titled by its page and marked by the site's own icon, as
  // a browser's is. Three states in order: the spinner while it loads, the
  // favicon once one has been found for that host, and the globe for a site
  // that has none or has not been asked yet — which is also every tab for the
  // first second of a cold start, since the icons are read from the database
  // rather than shipped with the app.
  const bid = browseId(pathname);
  if (bid != null) {
    const tab = browserTabs.find((t) => t.id === bid);
    const icon = faviconFor(tab?.url, favicons);
    return {
      title: tab?.title || hostOf(tab?.url ?? "") || "New tab",
      icon: tab?.loading ? (
        <ArrowClockwise size={size} className="animate-spin" />
      ) : icon ? (
        <img
          src={icon}
          alt=""
          width={size}
          height={size}
          /* A square box whatever the icon's own aspect: `object-contain`
             letterboxes a wide one rather than cropping it, and the fixed
             size keeps every tab's title starting at the same x. */
          style={{ width: size, height: size }}
          className="shrink-0 rounded-[2px] object-contain"
        />
      ) : (
        <Globe size={size} />
      ),
    };
  }
  // Exact, not a prefix: every other route starts with "/" too.
  if (pathname === "/") return { title: "Home", icon: <House size={size} /> };
  // A tab that has not been sent anywhere yet, titled as a browser's is.
  if (pathname === "/new")
    return { title: "New tab", icon: <FileDashed size={size} /> };
  // A conversation is titled by itself, like a project or a lecture: the name
  // rides in the query, written by `ChatPage` as the open thread changes and
  // as the model renames it. Nothing open means no `?n=`, and the tab is the
  // section it is.
  if (pathname.startsWith("/chat"))
    return {
      title: new URLSearchParams(search).get("n") || "Chat",
      icon: <Chat size={size} />,
    };
  if (pathname.startsWith("/calendar"))
    return { title: "Calendar", icon: <CalendarBlank size={size} /> };
  // A task's own page, tested *before* the project below it: `/projects/(\d+)`
  // is a prefix match and would otherwise swallow the task route and title it
  // with the project's `?n=`. A single item rather than a board, so a
  // checkbox rather than the Kanban glyph.
  if (/^\/projects\/\d+\/tasks\/\d+/.test(pathname)) {
    return {
      title: new URLSearchParams(search).get("n") || "Task",
      icon: <CheckSquare size={size} />,
    };
  }
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
  // An unfiled task's own page — the `/tasks/:taskId` half of `taskHref`.
  // Tested before the universal list below it for the same reason the project
  // pair above is ordered that way: `/tasks` is a prefix of this path.
  if (/^\/tasks\/\d+/.test(pathname)) {
    return {
      title: new URLSearchParams(search).get("n") || "Task",
      icon: <CheckSquare size={size} />,
    };
  }
  // Every task across every project, and the ones filed nowhere. A checklist
  // rather than the single task's checkbox or the board's Kanban glyph — it is
  // a list, and it is not one project's.
  if (pathname.startsWith("/tasks"))
    return { title: "Tasks", icon: <ListChecks size={size} /> };
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
