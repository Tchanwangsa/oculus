import { useEffect } from "react";
import { useSidePanelStore } from "@/stores/sidePanelStore";
import { useActivePath } from "@/stores/tabStore";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { PanelHeader } from "@/components/panel/PanelHeader";
import { FileViewer, PdfMdToggle, usePdfMd } from "@/components/files/FileViewer";
import { MarkdownUnavailable } from "@/components/files/ParseState";
import { filePagePath, fileTitle, openFileSmart } from "@/lib/openFile";
import { recordRecent } from "@/lib/recents";
import type { DbFile } from "@/lib/db";

/** A file open in the side panel; `SidePanel` owns the expand-to-page move. */
export default function FilePanel({
  file,
  paneId,
  onExpand,
}: {
  file: DbFile;
  paneId: number;
  onExpand: (path: string, newTab: boolean) => void;
}) {
  const { files } = useSubjectFiles(file.subject_id);
  const pdf = usePdfMd(file);
  const here = useActivePath();
  const close = useSidePanelStore((s) => s.close);

  // Navigating the front tab out of the file's subject closes it.
  useEffect(() => {
    const m = /^\/subjects\/(\d+)/.exec(here);
    if (!m || m[1] !== String(file.subject_id)) close(paneId);
  }, [here, file.subject_id, close, paneId]);

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
        onClose={() => close(paneId)}
        actions={
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
