import { useEffect, useMemo, useRef, useState } from "react";
import { Navigate, useNavigate, useParams, useSearchParams } from "react-router-dom";
import { CircleNotch } from "@phosphor-icons/react";
import {
  DocumentControls,
  DocumentEditor,
  type EditorMode,
  type SaveStatus,
} from "@/components/documents/DocumentEditor";
import { FileViewer, PdfMdToggle, usePdfMd } from "@/components/files/FileViewer";
import { MarkdownUnavailable } from "@/components/files/ParseState";
import { SubjectCrumbs, fileCrumbTab } from "@/components/subjects/SubjectCrumbs";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { filePagePath, fileTitle, openFileSmart, recordFileAccess } from "@/lib/openFile";

/**
 * A file as a full page — what the peek's expand button promotes into its own
 * tab. Standalone (not under SubjectLayout): like Notion, a full-page document
 * takes the entire content area, so the subject's title block and tab strip
 * are not above it. Its breadcrumb is what stands in for them.
 *
 * For one of the student's own documents this page *is* the editor: the same
 * header, with the Write / Preview pills and the save word where a PDF would
 * have its toggle, and `DocumentEditor` below instead of the viewer.
 */
export default function SubjectFilePage() {
  const { subjectId } = useParams();
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const relPath = searchParams.get("path");
  const id = Number(subjectId);

  const { files, loading } = useSubjectFiles(Number.isFinite(id) ? id : null);
  const found = useMemo(
    () => files.find((f) => f.relative_path === relPath) ?? null,
    [files, relPath],
  );

  // A document renamed under this page keeps its row and loses its path. The
  // row is followed by id: when the path in the route has gone but the row is
  // still here under another, the route is brought up to date rather than
  // the page given up — so the editor stays mounted across its own rename
  // and the tab follows the file.
  // Only a document is followed this way: a stale path to anything else — a
  // re-synced download, a deleted page — still lands on the subject rather
  // than bouncing back to whatever this pane showed before.
  const heldId = useRef<number | null>(null);
  if (found) heldId.current = found.category === "document" ? found.id : null;
  const moved = useMemo(
    () =>
      found || heldId.current == null
        ? null
        : files.find((f) => f.id === heldId.current && f.category === "document") ?? null,
    [found, files],
  );
  const file = found ?? moved;
  useEffect(() => {
    if (moved) navigate(filePagePath(moved.subject_id, moved.relative_path), { replace: true });
  }, [moved, navigate]);

  const pdf = usePdfMd(file);
  const isDocument = file?.category === "document";
  const [mode, setMode] = useState<EditorMode>("write");
  const [status, setStatus] = useState<SaveStatus>({ state: "idle" });

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
      {/* One header: where the file sits, its title, and the PDF ↔ Markdown
          toggle when it applies. The trail is the page's only subject chrome —
          without it a file opened from Home or the palette names no subject
          and offers no way back to the list it came from. */}
      <div className="h-11 shrink-0 flex items-center gap-2.5 px-5 border-b border-border-subtle">
        <nav
          aria-label="Breadcrumb"
          className="flex shrink-0 items-center gap-2.5 text-[11px] text-muted-foreground"
        >
          <SubjectCrumbs subjectId={id} tab={fileCrumbTab(file.category)} />
        </nav>
        <h1 className="flex-1 min-w-0 text-[13px] font-semibold text-foreground truncate">
          {fileTitle(file)}
        </h1>
        {/* A document carries its editor's controls. A PDF with markdown
            toggles; a PDF without says why it has none, rather than quietly
            lacking the control the reader saw on the last file
            (`MarkdownUnavailable`). */}
        {isDocument ? (
          <DocumentControls mode={mode} onMode={setMode} status={status} />
        ) : (
          pdf.isPdf && pdf.mdChecked &&
          (pdf.mdExists ? (
            <PdfMdToggle value={pdf.viewMode} onChange={pdf.setViewMode} />
          ) : (
            <MarkdownUnavailable file={file} />
          ))
        )}
      </div>
      <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
        {isDocument ? (
          <DocumentEditor
            file={file}
            files={files}
            mode={mode}
            onMode={setMode}
            onStatus={setStatus}
          />
        ) : (
          /* In-document links open the linked file as a peek over this page. */
          <FileViewer
            file={file}
            files={files}
            onOpenFile={openFileSmart}
            pdfViewMode={pdf.viewMode}
          />
        )}
      </div>
    </div>
  );
}
