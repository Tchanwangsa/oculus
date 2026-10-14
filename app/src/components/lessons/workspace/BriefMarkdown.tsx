import { useLayoutEffect, useMemo, useRef } from "react";
import type { Components } from "react-markdown";

import { useLibraryMdComponents } from "@/components/files/FileViewer";
import { FileMarkdown } from "@/components/files/FileMarkdown";
import { EmbedSubjectProvider } from "@/components/markdown/embed/EmbedSubject";
import { FileChip } from "@/components/markdown/FileChip";
import type { DbFile } from "@/lib/db";
import { dropInlineFiles, workspaceFilePath } from "@/lib/ed/briefFiles";
import { openFileSmart, openLibraryPath } from "@/lib/files/openFile";

/**
 * The challenge slide's `.md` as the workspace's Description: the brief and
 * its Files list, without the inline copy of each file the editor already
 * shows (`dropInlineFiles`). A file in that list opens as its editor tab;
 * one the workspace does not list, or a ⌘-click, opens the library copy.
 */
export function BriefMarkdown({
  brief,
  files,
  openWorkspaceFile,
}: {
  brief: DbFile;
  files: DbFile[];
  /** Opens `path`'s tab and says so; false when the workspace has no such file. */
  openWorkspaceFile: (path: string) => boolean;
}) {
  const base = useLibraryMdComponents(brief, files, openFileSmart);
  // The latest tabs, read at click time, so the markdown never re-renders
  // for a status change in the pane.
  const open = useRef(openWorkspaceFile);
  useLayoutEffect(() => {
    open.current = openWorkspaceFile;
  });

  const components = useMemo<Components>(() => {
    const stem = brief.relative_path.replace(/\.md$/, "");
    return {
      ...base,
      a: (props) => {
        const path = workspaceFilePath(props.href ?? "", brief.relative_path);
        if (path == null) return base.a(props);
        const library = `${stem}/${path}`;
        return (
          <FileChip
            path={library}
            onClick={(newTab) => {
              if (!newTab && open.current(path)) return;
              openLibraryPath(library, newTab);
            }}
          >
            {props.children}
          </FileChip>
        );
      },
    };
  }, [base, brief.relative_path]);

  return (
    <EmbedSubjectProvider subjectId={brief.subject_id}>
      <FileMarkdown relPath={brief.relative_path} components={components} transform={dropInlineFiles} />
    </EmbedSubjectProvider>
  );
}
