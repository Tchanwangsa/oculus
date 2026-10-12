import { lazy, Suspense, useEffect, useMemo, useRef, useState } from "react";
import { Navigate, useLocation, useNavigate, useParams, useSearchParams } from "react-router-dom";
import { ArrowSquareOut, Globe } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { isWebUrl, openExternal } from "@/lib/browser";
import { isEdUrl } from "@/lib/ed/links";
import {
  DocumentControls,
  SUGGEST_IDLE,
  type DocumentActions,
  type EditorMode,
  type SaveStatus,
  type SuggestStatus,
} from "@/components/documents/DocumentControls";
import { FileViewer, PdfMdToggle, usePdfMd } from "@/components/files/FileViewer";
import { MarkdownUnavailable } from "@/components/files/ParseState";
import { SubjectCrumbs, fileCrumbTab } from "@/components/subjects/SubjectCrumbs";
import { LessonView } from "@/components/lessons/LessonView";
import { useLessonIndex } from "@/hooks/data/useLessonIndex";
import { useSubjectFiles } from "@/hooks/data/useSubjectFiles";
import {
  filePagePath,
  fileTitle,
  openFileSmart,
  recordFileAccess,
  routeLocate,
  routeParseDetails,
} from "@/lib/files/openFile";
import { useLocateHighlight } from "@/hooks/find/useLocateHighlight";
import { LoadingFill } from "@/components/ui/layout/PageParts";
import { PaneHeaderRow, PaneTitle, PaneTrail } from "@/components/tabs/PaneHeader";
import { useDocumentPrefsStore } from "@/stores/documents/documentPrefsStore";

const DocumentEditor = lazy(() =>
  import("@/components/documents/DocumentEditor").then((m) => ({ default: m.DocumentEditor })),
);

/**
 * A file as a full page, in a tab or in the side panel (`openFileSmart`).
 * Standalone, not under SubjectLayout; its breadcrumb stands in for the
 * subject chrome. For a student document this page is the editor
 * (`DocumentEditor`).
 */
export default function SubjectFilePage() {
  const { subjectId } = useParams();
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const relPath = searchParams.get("path");
  const id = Number(subjectId);
  // A cited spot (`openCitation`), from router state: a page in the PDF, or a
  // passage of markdown to highlight.
  const { state: routeState } = useLocation();
  const locate = useMemo(() => routeLocate(routeState), [routeState]);
  const parseDetails = routeParseDetails(routeState);

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
  // An Ed lesson whose index lists its slides opens as the lesson view.
  const lesson = useLessonIndex(file);
  const isLessonView = !!lesson.index && lesson.index.slides.length > 0;

  // A cited page is in the PDF: the Markdown face gives way to it.
  const { setViewMode } = pdf;
  useEffect(() => {
    if (locate?.page) setViewMode("pdf");
  }, [locate?.seq, locate?.page, setViewMode]);
  // Not in a document: its editor draws only the lines in view, so the
  // passage may not be in the DOM to find.
  const [body, setBody] = useState<HTMLDivElement | null>(null);
  useLocateHighlight(body, isDocument ? undefined : locate);

  const [mode, setMode] = useState<EditorMode>("live");
  const [status, setStatus] = useState<SaveStatus>({ state: "idle" });
  const suggestions = useDocumentPrefsStore((s) => s.suggestions);
  const setSuggestions = useDocumentPrefsStore((s) => s.setSuggestions);
  const loadDocumentPrefs = useDocumentPrefsStore((s) => s.load);
  const [suggestStatus, setSuggestStatus] = useState<SuggestStatus>(SUGGEST_IDLE);
  const [history, setHistory] = useState(false);
  /** The mounted editor's, for the header's Save version. */
  const documentActions = useRef<DocumentActions | null>(null);
  const saveVersion = (label?: string) =>
    documentActions.current
      ? documentActions.current.saveVersion(label)
      : Promise.reject(new Error("The note is still loading."));
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
  const fileSubjectId = file?.subject_id;
  useEffect(() => {
    if (fileId != null) recordFileAccess({ id: fileId, subject_id: fileSubjectId });
  }, [fileId, fileSubjectId]);

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
      <PaneHeaderRow className="h-11 shrink-0 flex items-center gap-2.5 px-5 border-b border-border-subtle">
        <PaneTrail>
          <nav
            aria-label="Breadcrumb"
            className="flex shrink-0 items-center gap-2.5 text-[11px] text-muted-foreground"
          >
            <SubjectCrumbs subjectId={id} tab={fileCrumbTab(file)} />
          </nav>
          <PaneTitle>{fileTitle(file)}</PaneTitle>
        </PaneTrail>
        {/* A PDF without markdown says why rather than lacking the toggle. */}
        {isDocument ? (
          <DocumentControls
            mode={mode}
            onMode={setMode}
            status={status}
            suggestions={suggestions}
            onSuggestions={toggleSuggestions}
            suggestStatus={suggestStatus}
            history={history}
            onHistory={setHistory}
            onSaveVersion={saveVersion}
          />
        ) : (
          pdf.isPdf && pdf.mdChecked &&
          (pdf.mdExists ? (
            <PdfMdToggle value={pdf.viewMode} onChange={pdf.setViewMode} />
          ) : (
            <MarkdownUnavailable file={file} openSeq={parseDetails} />
          ))
        )}
        {/* A synced page is our markdown of a Canvas page or an Ed thread;
            this opens the original, embeds and styling included, in a
            browser tab. The lesson view has its own, for the shown slide. */}
        {!isDocument && !isLessonView && isWebUrl(file.source_url) && (
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label={isEdUrl(file.source_url) ? "Open in Ed" : "Open in browser"}
            title={isEdUrl(file.source_url) ? "Open in Ed" : "Open in browser"}
            onClick={(e) => void openExternal(file.source_url!, e.metaKey)}
            className="shrink-0 text-muted-foreground hover:text-foreground"
          >
            {isEdUrl(file.source_url) ? <ArrowSquareOut size={14} /> : <Globe size={14} />}
          </Button>
        )}
      </PaneHeaderRow>
      <div ref={setBody} className="flex-1 min-h-0 overflow-hidden flex flex-col">
        {isDocument ? (
          <Suspense fallback={<LoadingFill />}>
            <DocumentEditor
              key={file.id}
              file={file}
              files={files}
              mode={mode}
              onMode={setMode}
              onStatus={setStatus}
              suggestions={suggestions}
              onSuggestStatus={setSuggestStatus}
              history={history}
              onHistory={setHistory}
              actions={documentActions}
            />
          </Suspense>
        ) : isLessonView && lesson.index ? (
          <LessonView
            file={file}
            index={lesson.index}
            files={files}
            title={lesson.index.title ?? fileTitle(file)}
          />
        ) : !lesson.checked ? (
          <LoadingFill />
        ) : (
          /* In-document links open the linked file beside this page. */
          <FileViewer
            file={file}
            files={files}
            onOpenFile={openFileSmart}
            pdfViewMode={pdf.viewMode}
            pdfLink={pdf.link}
            locate={locate}
          />
        )}
      </div>
    </div>
  );
}
