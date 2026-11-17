import { useMemo } from "react";
import { useNavigate } from "react-router-dom";
import type { DbFile } from "@/lib/db";
import { filePageHref, openFileSmart } from "@/lib/files/openFile";
import { lecturePageId } from "@/lib/lectures/pagePath";
import { openBeside, openLink } from "@/lib/shell/tabRouters";

export interface OverviewOpener {
  /** The library row an item's href names, for its glyph and how it opens. */
  fileFor: (href: string) => DbFile | undefined;
  open: (href: string) => void;
  /** A row's `data-tab-href` (⌘-click), absent for web links and system-only files. */
  tabHref: (href: string) => string | undefined;
}

const isWeb = (href: string) => /^https?:/i.test(href);

function filePath(href: string): string | null {
  const match = /^\/subjects\/\d+\/file\?(.*)$/.exec(href);
  return match ? new URLSearchParams(match[1]).get("path") : null;
}

/**
 * How Overview rows open, as the subject's other tabs open the same things:
 * library files through `openFileSmart` (side panel, or the system viewer for
 * a binary the app can't show), lectures beside the page, Canvas links in the
 * in-app browser, and chats or tasks in this pane.
 */
export function useOverviewOpener(files: DbFile[]): OverviewOpener {
  const navigate = useNavigate();
  const byPath = useMemo(() => new Map(files.map((file) => [file.relative_path, file])), [files]);
  return useMemo(() => {
    const fileFor = (href: string) => {
      const path = filePath(href);
      return path == null ? undefined : byPath.get(path);
    };
    return {
      fileFor,
      open: (href) => {
        if (isWeb(href)) return void openLink(href);
        const file = fileFor(href);
        if (file) return openFileSmart(file);
        if (filePath(href) != null || lecturePageId(href)) return openBeside(href);
        navigate(href);
      },
      tabHref: (href) => {
        if (isWeb(href)) return undefined;
        const file = fileFor(href);
        return file ? filePageHref(file) ?? undefined : href;
      },
    };
  }, [byPath, navigate]);
}
