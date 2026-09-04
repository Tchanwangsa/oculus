import { useEffect } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { usePeekStore } from "@/stores/peekStore";
import { useTabStore } from "@/stores/tabStore";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { PeekPanel } from "@/components/peek/PeekPanel";
import { FileViewer, PdfMdToggle, usePdfMd } from "@/components/files/FileViewer";
import { fileTitle, openFileSmart } from "@/lib/openFile";
import { recordRecent } from "@/lib/recents";

/** Builds the full-page route for a file. */
export function filePagePath(subjectId: number, relativePath: string): string {
  return `/subjects/${subjectId}/file?path=${encodeURIComponent(relativePath)}`;
}

/**
 * The app-wide file peek, anchored in AppLayout so it overlays the entire page
 * under the tab strip. Expanding promotes the file to a full page in a new tab.
 */
export default function FilePeek() {
  const { file, close } = usePeekStore();
  const { files } = useSubjectFiles(file?.subject_id ?? null);
  const pdf = usePdfMd(file);
  const location = useLocation();
  const navigate = useNavigate();
  const addTab = useTabStore((s) => s.addTab);

  // A peek belongs to the subject it was opened in — navigating to another
  // subject (or out of subjects entirely) closes it.
  useEffect(() => {
    if (!file) return;
    const m = /^\/subjects\/(\d+)/.exec(location.pathname);
    if (!m || m[1] !== String(file.subject_id)) close();
  }, [location.pathname, file, close]);

  // Feeds the "Recently visited" row on the subject home.
  useEffect(() => {
    if (!file) return;
    recordRecent(file.subject_id, {
      kind: "file",
      ref: file.relative_path,
      title: fileTitle(file),
      category: file.category ?? undefined,
    });
  }, [file]);

  if (!file) return null;

  const openAsPage = () => {
    const to = filePagePath(file.subject_id, file.relative_path);
    addTab(to);
    navigate(to);
    close();
  };

  return (
    // Keyed so switching files mid-close remounts a fresh panel (and replays
    // the slide-in) instead of inheriting the closing animation.
    <PeekPanel
      key={file.relative_path}
      title={fileTitle(file)}
      onExpand={openAsPage}
      onClose={close}
      actions={
        pdf.isPdf && pdf.mdExists ? (
          <PdfMdToggle value={pdf.viewMode} onChange={pdf.setViewMode} />
        ) : undefined
      }
    >
      <FileViewer
        file={file}
        files={files}
        onOpenFile={openFileSmart}
        pdfViewMode={pdf.viewMode}
      />
    </PeekPanel>
  );
}
