import { useEffect, useMemo } from "react";
import { Navigate, useParams, useSearchParams } from "react-router-dom";
import { CircleNotch } from "@phosphor-icons/react";
import { FileViewer, PdfMdToggle, usePdfMd } from "@/components/files/FileViewer";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { fileTitle, openFileSmart, recordFileAccess } from "@/lib/openFile";

/**
 * A file as a full page — what the peek's expand button promotes into its own
 * tab. Standalone (not under SubjectLayout): like Notion, a full-page document
 * takes the entire content area, no subject chrome.
 */
export default function SubjectFilePage() {
  const { subjectId } = useParams();
  const [searchParams] = useSearchParams();
  const relPath = searchParams.get("path");
  const id = Number(subjectId);

  const { files, loading } = useSubjectFiles(Number.isFinite(id) ? id : null);
  const file = useMemo(
    () => files.find((f) => f.relative_path === relPath) ?? null,
    [files, relPath],
  );
  const pdf = usePdfMd(file);

  // Direct navigation (restored tab, deep link) bypasses openFileSmart, so
  // record the access here. Keyed on id: the refresh the record triggers
  // replaces `file` with an equal object and must not re-stamp.
  const fileId = file?.id;
  useEffect(() => {
    if (fileId != null) recordFileAccess({ id: fileId });
  }, [fileId]);

  if (!Number.isFinite(id) || !relPath) return <Navigate to="/subjects" replace />;

  if (!file) {
    if (loading || files.length === 0) {
      return (
        <div className="h-full flex items-center justify-center gap-2 text-muted-foreground">
          <CircleNotch size={16} className="animate-spin" />
          <span className="text-sm">Loading…</span>
        </div>
      );
    }
    // Files loaded but the path is gone — stale tab after a re-sync.
    return <Navigate to={`/subjects/${id}`} replace />;
  }

  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* One header: title + the PDF ↔ Markdown toggle when it applies. */}
      <div className="h-11 shrink-0 flex items-center gap-3 px-5 border-b border-border-subtle">
        <h1 className="flex-1 min-w-0 text-[13px] font-semibold text-foreground truncate">
          {fileTitle(file)}
        </h1>
        {pdf.isPdf && pdf.mdExists && (
          <PdfMdToggle value={pdf.viewMode} onChange={pdf.setViewMode} />
        )}
      </div>
      <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
        {/* In-document links open the linked file as a peek over this page. */}
        <FileViewer
          file={file}
          files={files}
          onOpenFile={openFileSmart}
          pdfViewMode={pdf.viewMode}
        />
      </div>
    </div>
  );
}
