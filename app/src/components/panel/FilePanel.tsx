import { useEffect } from "react";
import { useSidePanelStore } from "@/stores/sidePanelStore";
import { useActivePath } from "@/stores/tabStore";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { PanelHeader } from "@/components/panel/PanelHeader";
import { FileViewer, PdfMdToggle, usePdfMd } from "@/components/files/FileViewer";
import { MarkdownUnavailable } from "@/components/files/ParseState";
import { fileTitle, openFileSmart } from "@/lib/openFile";
import { recordRecent } from "@/lib/recents";
import type { DbFile } from "@/lib/db";

/** Builds the full-page route for a file. */
export function filePagePath(subjectId: number, relativePath: string): string {
  return `/subjects/${subjectId}/file?path=${encodeURIComponent(relativePath)}`;
}

/**
 * A file open in the side panel. Expanding promotes it to a full page —
 * in this tab, or in a new one on ⌘-click. The panel owns that move (see
 * `SidePanel`) because it is the panel that animates through it; all this
 * knows is the route.
 */
export default function FilePanel({
  file,
  tabId,
  onExpand,
}: {
  file: DbFile;
  tabId: number;
  onExpand: (path: string, newTab: boolean) => void;
}) {
  const { files } = useSubjectFiles(file.subject_id);
  const pdf = usePdfMd(file);
  const here = useActivePath();
  const close = useSidePanelStore((s) => s.close);

  // A file belongs to the subject it was opened in — navigating to another
  // subject (or out of subjects entirely) closes it. Only the tab in front is
  // checked: it is the only one whose route the user is steering, and a
  // background tab that comes forward is still sitting where it was left.
  useEffect(() => {
    const m = /^\/subjects\/(\d+)/.exec(here);
    if (!m || m[1] !== String(file.subject_id)) close(tabId);
  }, [here, file.subject_id, close, tabId]);

  // Feeds the "Recently visited" row on the subject home.
  useEffect(() => {
    recordRecent(file.subject_id, {
      kind: "file",
      ref: file.relative_path,
      title: fileTitle(file),
      category: file.category ?? undefined,
    });
  }, [file]);

  return (
    <>
      <PanelHeader
        title={fileTitle(file)}
        onExpand={(newTab) =>
          onExpand(filePagePath(file.subject_id, file.relative_path), newTab)
        }
        onClose={() => close(tabId)}
        actions={
          /* Same pairing as the full page: the toggle when there is markdown,
             the reason there is none when there is not. */
          pdf.isPdf && pdf.mdChecked ? (
            pdf.mdExists ? (
              <PdfMdToggle value={pdf.viewMode} onChange={pdf.setViewMode} />
            ) : (
              <MarkdownUnavailable file={file} />
            )
          ) : undefined
        }
      />
      <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
        <FileViewer
          file={file}
          files={files}
          onOpenFile={openFileSmart}
          pdfViewMode={pdf.viewMode}
        />
      </div>
    </>
  );
}
