import { useEffect } from "react";
import { useSidePanelStore } from "@/stores/sidePanelStore";
import { useActivePath, useTabStore } from "@/stores/tabStore";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { PanelHeader } from "@/components/panel/PanelHeader";
import { FileViewer, PdfMdToggle, usePdfMd } from "@/components/files/FileViewer";
import { fileTitle, openFileSmart } from "@/lib/openFile";
import { recordRecent } from "@/lib/recents";
import type { DbFile } from "@/lib/db";

/** Builds the full-page route for a file. */
export function filePagePath(subjectId: number, relativePath: string): string {
  return `/subjects/${subjectId}/file?path=${encodeURIComponent(relativePath)}`;
}

/** A file open in the side panel. Expanding promotes it to its own tab. */
export default function FilePanel({ file, tabId }: { file: DbFile; tabId: number }) {
  const { files } = useSubjectFiles(file.subject_id);
  const pdf = usePdfMd(file);
  const here = useActivePath();
  const addTab = useTabStore((s) => s.addTab);
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
        onExpand={() => {
          addTab(filePagePath(file.subject_id, file.relative_path));
          close(tabId);
        }}
        onClose={() => close(tabId)}
        actions={
          pdf.isPdf && pdf.mdExists ? (
            <PdfMdToggle value={pdf.viewMode} onChange={pdf.setViewMode} />
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
