import { useCallback } from "react";
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
import { useSubjects } from "@/hooks/useSubjects";
import { useBrowserStore } from "@/stores/browserStore";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode, humanizeSlug } from "@/lib/format";
import { SETTINGS_PAGES } from "@/lib/settingsSearch";
import type { Subject } from "@/lib/db";

const SECTION_LABELS: Record<string, string> = {
  modules: "Modules",
  files: "Files",
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
 * A route's title and icon, shared by the tab strip and the new-tab page's
 * Recent list. Derived from the path on every render, so renames follow on
 * their own.
 */
export function tabInfo(
  path: string,
  subjects: Subject[],
  browserTabs: BrowserTab[],
  size = 13,
  favicons: Record<string, string> = {},
): TabInfo {
  const [pathname, search = ""] = path.split("?");
  // Browser tab icon: spinner while loading, else the site favicon, else a globe.
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
          style={{ width: size, height: size }}
          className="shrink-0 rounded-[2px] object-contain"
        />
      ) : (
        <Globe size={size} />
      ),
    };
  }
  if (pathname === "/") return { title: "Home", icon: <House size={size} /> };
  if (pathname === "/new")
    return { title: "New tab", icon: <FileDashed size={size} /> };
  // Chat, project and task titles ride in `?n=` (this has no lists to look up).
  if (pathname.startsWith("/chat"))
    return {
      title: new URLSearchParams(search).get("n") || "Chat",
      icon: <Chat size={size} />,
    };
  if (pathname.startsWith("/calendar"))
    return { title: "Calendar", icon: <CalendarBlank size={size} /> };
  // Task routes are tested before their prefix routes (project, task list).
  if (/^\/projects\/\d+\/tasks\/\d+/.test(pathname)) {
    return {
      title: new URLSearchParams(search).get("n") || "Task",
      icon: <CheckSquare size={size} />,
    };
  }
  const proj = /^\/projects\/(\d+)/.exec(pathname);
  if (proj) {
    return {
      title: new URLSearchParams(search).get("n") || "Project",
      icon: <Kanban size={size} />,
    };
  }
  if (pathname.startsWith("/projects"))
    return { title: "Projects", icon: <Kanban size={size} /> };
  if (/^\/tasks\/\d+/.test(pathname)) {
    return {
      title: new URLSearchParams(search).get("n") || "Task",
      icon: <CheckSquare size={size} />,
    };
  }
  if (pathname.startsWith("/tasks"))
    return { title: "Tasks", icon: <ListChecks size={size} /> };
  if (pathname.startsWith("/sync"))
    return { title: "Sync", icon: <ArrowsClockwise size={size} /> };
  if (pathname.startsWith("/settings")) {
    // Named for its page (`/settings/opencode` is "opencode"); the bare route,
    // which redirects, stays "Settings".
    const page = SETTINGS_PAGES.find((p) => p.id === pathname.split("/")[2]);
    return { title: page?.label ?? "Settings", icon: <GearSix size={size} /> };
  }
  const m = /^\/subjects\/(\d+)(?:\/([\w-]+))?/.exec(pathname);
  if (m) {
    const subject = subjects.find((s) => String(s.id) === m[1]);
    const icon = subject ? (
      <SubjectIcon code={subject.code} size={size} />
    ) : (
      <BookOpen size={size} />
    );
    // A file is titled by its name; the student's own notes stay verbatim
    // (humanising would title-case them and eat a leading date).
    if (m[2] === "file") {
      const rel = new URLSearchParams(search).get("path");
      const base = rel?.split("/").pop();
      if (base && rel?.includes("/documents/"))
        return { title: base.replace(/\.md$/i, ""), icon };
      if (base)
        return { title: humanizeSlug(base.replace(/\.pdf$/i, "")), icon };
    }
    if (m[2] === "lecture") {
      return { title: new URLSearchParams(search).get("t") ?? "Lecture", icon };
    }
    const code = subject ? displayCode(subject.code) : "Subject";
    // The first segment only: a sub-tab of Files is still the Files tab.
    const section = m[2] ? SECTION_LABELS[m[2]] : null;
    return { title: section ? `${code} · ${section}` : code, icon };
  }
  if (pathname.startsWith("/subjects"))
    return { title: "Subjects", icon: <BookOpen size={size} /> };
  return { title: "Oculus", icon: null };
}

/** `tabInfo` over the app's live subjects, browser pages and favicons: what
 *  the tab strip and the side panel's header title their panes with. */
export function useTabInfo(): (path: string, size?: number) => TabInfo {
  const { subjects } = useSubjects();
  const browserTabs = useBrowserStore((s) => s.tabs);
  const favicons = useBrowserStore((s) => s.favicons);
  return useCallback(
    (path, size = 13) => tabInfo(path, subjects, browserTabs, size, favicons),
    [subjects, browserTabs, favicons],
  );
}
