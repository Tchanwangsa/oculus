import { useEffect, useMemo, useRef, useState } from "react";
import { Navigate, useNavigate, useParams, useSearchParams } from "react-router-dom";
import {
  DocumentControls,
  DocumentEditor,
  type EditorMode,
  type SaveStatus,
} from "@/components/documents/DocumentEditor";
import { FileViewer, PdfMdToggle, usePdfMd } from "@/components/files/FileViewer";
import { MarkdownUnavailable } from "@/components/files/ParseState";
import { SUGGEST_IDLE, type SuggestStatus } from "@/components/documents/editor/aiSuggest";
import { SubjectCrumbs, fileCrumbTab } from "@/components/subjects/SubjectCrumbs";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { filePagePath, fileTitle, openFileSmart, recordFileAccess } from "@/lib/openFile";
import { LoadingFill } from "@/components/ui/PageParts";
import { useDocumentPrefsStore } from "@/stores/documentPrefsStore";

/**
 * A file as a full page (the peek's expand target). Standalone, not under
 * SubjectLayout; its breadcrumb stands in for the subject chrome. For a
 * student document this page is the editor (`DocumentEditor`).
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

  // A document renamed under this page keeps its row but loses its path:
  // follow the row by id and update the route, so the editor stays mounted.
  // Only documents; any other stale path falls through to the subject.
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
  const [mode, setMode] = useState<EditorMode>("live");
  const [status, setStatus] = useState<SaveStatus>({ state: "idle" });
  const suggestions = useDocumentPrefsStore((s) => s.suggestions);
  const setSuggestions = useDocumentPrefsStore((s) => s.setSuggestions);
  const loadDocumentPrefs = useDocumentPrefsStore((s) => s.load);
  const [suggestStatus, setSuggestStatus] = useState<SuggestStatus>(SUGGEST_IDLE);
  useEffect(() => {
    if (isDocument) void loadDocumentPrefs();
  }, [isDocument, loadDocumentPrefs]);
  const toggleSuggestions = (on: boolean) => {
    setSuggestions(on).catch(console.error);
  };

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
        <LoadingFill />
      );
    }
    // Files loaded but the path is gone — stale tab after a re-sync.
    return <Navigate to={`/subjects/${id}`} replace />;
  }

  return (
    <div className="h-full flex flex-col overflow-hidden">
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
        {/* A PDF without markdown says why rather than lacking the toggle. */}
        {isDocument ? (
          <DocumentControls
            mode={mode}
            onMode={setMode}
            status={status}
            suggestions={suggestions}
            onSuggestions={toggleSuggestions}
            suggestStatus={suggestStatus}
          />
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
            key={file.id}
            file={file}
            files={files}
            mode={mode}
            onMode={setMode}
            onStatus={setStatus}
            suggestions={suggestions}
            onSuggestStatus={setSuggestStatus}
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
