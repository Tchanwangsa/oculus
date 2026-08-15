import {
  BookOpenIcon,
  DocumentTextIcon,
  DocumentIcon,
  ChevronRightIcon,
  ChevronDownIcon,
  ArrowPathIcon,
  PhotoIcon,
  MegaphoneIcon,
  PaperClipIcon,
  ArrowTopRightOnSquareIcon,
  Square3Stack3DIcon,
  CodeBracketSquareIcon,
} from "@heroicons/react/16/solid";
import { ArrowPathIcon as ArrowPathMdIcon } from "@heroicons/react/20/solid";
import {
  ExclamationTriangleIcon,
  ArrowTopRightOnSquareIcon as ArrowTopRightOnSquareLgIcon,
} from "@heroicons/react/24/outline";
import { useState, useEffect, useCallback, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeRaw from "rehype-raw";
import rehypeKatex from "rehype-katex";
import "katex/dist/katex.min.css";
import { upsertFile, setParseStatusByPath, type DbFile } from "@/lib/db";
import { useDataDir } from "@/hooks/useDataDir";
import { useSubjects } from "@/hooks/useSubjects";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { useFileContent } from "@/hooks/useFileContent";
import { useParseStore } from "@/stores/parseStore";
import { useResizablePanel } from "@/hooks/useResizablePanel";
import { ResizeHandle } from "@/components/ui/ResizeHandle";
import { MD_COMPONENTS } from "@/components/markdown/MdComponents";
import { CourseRow, courseColor } from "@/components/subjects/CourseRow";
import { FileCategorySection } from "@/components/files/FileCategorySection";
import { FileRow } from "@/components/files/FileRow";
import { PDFViewer } from "@/components/files/PDFViewer";

// ── MdFromPath ────────────────────────────────────────────────────────────────

function MdFromPath({
  relPath, components, qualityStatus, pagesDone, totalPages,
}: {
  relPath: string;
  components: any;
  qualityStatus?: string;
  pagesDone?: number;
  totalPages?: number;
}) {
  const [text, setText] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    setText(null);
    setErr(null);
    invoke<string>("read_course_file", { relativePath: relPath })
      .then(setText)
      .catch((e) => setErr(String(e)));
  }, [relPath]);

  const showBanner = qualityStatus === "queued" || qualityStatus === "running";
  const bannerLabel =
    qualityStatus === "queued"
      ? "queued"
      : totalPages
      ? `${pagesDone ?? 0}/${totalPages} pages`
      : "starting…";

  if (err) return (
    <div className="px-8 py-6 text-xs text-destructive">Failed to load markdown: {err}</div>
  );
  if (text === null) return (
    <div className="h-full flex items-center justify-center gap-2 text-muted-foreground">
      <ArrowPathMdIcon className="size-[16px] animate-spin" /><span className="text-sm">Loading…</span>
    </div>
  );
  return (
    <div className="flex-1 overflow-y-auto">
      {showBanner && (
        <div className="mx-8 mt-5 px-3 py-2 rounded-md bg-amber-500/10 border border-amber-500/20 flex items-center gap-2 text-[11px] text-amber-600 dark:text-amber-400">
          <ArrowPathIcon className="size-[10px] animate-spin shrink-0" />
          Fast preview · Quality parse {bannerLabel}
        </div>
      )}
      <article className="markdown-body px-8 py-6 max-w-3xl">
        <ReactMarkdown remarkPlugins={[remarkGfm, remarkMath]} rehypePlugins={[rehypeRaw, rehypeKatex]} components={components}>
          {text}
        </ReactMarkdown>
      </article>
    </div>
  );
}

// ── Component ─────────────────────────────────────────────────────────────────

export default function SubjectsPage() {
  const dataDir = useDataDir();
  const { subjects, loading, current, past } = useSubjects();
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [pastExpanded, setPastExpanded] = useState(false);

  const { files, loading: filesLoading, byCategory, reload: reloadFiles } = useSubjectFiles(selectedId);
  const { activeFile, content, loading: contentLoading, error: contentError, assetUrl, openFile } =
    useFileContent(dataDir);

  const [imagesExpanded, setImagesExpanded] = useState(false);
  const [announcementsExpanded, setAnnouncementsExpanded] = useState(false);
  const [filesExpanded, setFilesExpanded] = useState(false);
  const [pagesExpanded, setPagesExpanded] = useState(false);
  const [rescraping, setRescraping] = useState<Set<number>>(new Set());
  const [rescrapeError, setRescrapeError] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<"pdf" | "markdown">("pdf");

  const subjectsPanel = useResizablePanel({
    defaultWidth: 220, minWidth: 120, maxWidth: 360,
    collapseThreshold: 80, storageKey: "subjects-panel",
  });
  const filesPanel = useResizablePanel({
    defaultWidth: 240, minWidth: 160, maxWidth: 420,
    collapseThreshold: 80, storageKey: "files-panel",
  });

  // Parse state from the global store (no per-file polling/SSE)
  const liveStatuses = useParseStore((s) => s.statuses);
  const parseJobs = useParseStore((s) => s.jobs);
  const mergeParseStatuses = useParseStore((s) => s.merge);

  // Reconcile parse status from disk when a subject's files load: PDFs with a
  // sibling .md (+ _images dir) get a badge even if parsed before status
  // tracking existed. Writes to the store (instant badge) and DB (persistence).
  useEffect(() => {
    const pdfPaths = files
      .filter((f) => f.filename.toLowerCase().endsWith(".pdf"))
      .map((f) => f.relative_path);
    if (pdfPaths.length === 0) return;
    invoke<Array<[string, string]>>("scan_parsed_files", { relativePaths: pdfPaths })
      .then((entries) => {
        if (entries.length === 0) return;
        mergeParseStatuses(Object.fromEntries(entries));
        setParseStatusByPath(entries).catch(() => {});
      })
      .catch(() => {});
  }, [files, mergeParseStatuses]);

  const isPdf = activeFile?.filename.toLowerCase().endsWith(".pdf") ?? false;
  const activeRelPath = isPdf && activeFile ? activeFile.relative_path : null;

  // Derive the open file's parse state: a live job (queued/running) takes
  // precedence, else the last-known terminal status string.
  const activeJob = activeRelPath ? parseJobs[activeRelPath] : undefined;
  const activeStatusStr = activeRelPath ? liveStatuses[activeRelPath] : undefined;
  const parseProgress = {
    status: activeJob
      ? activeJob.status // "queued" | "running"
      : activeStatusStr === "quality"
      ? "done"
      : activeStatusStr === "error"
      ? "error"
      : "idle",
    pages_done: activeJob?.pages_done,
    total_pages: activeJob?.total_pages,
  };

  const parsePct = parseProgress.total_pages
    ? Math.round((parseProgress.pages_done ?? 0) / parseProgress.total_pages * 100)
    : 0;
  const mdRelPath = isPdf && activeFile
    ? activeFile.relative_path.replace(/\.pdf$/i, ".md")
    : null;

  const [mdExists, setMdExists] = useState(false);
  useEffect(() => {
    setMdExists(false);
    if (!mdRelPath) return;
    invoke<string>("read_course_file", { relativePath: mdRelPath })
      .then((t) => setMdExists(t.length > 0))
      .catch(() => setMdExists(false));
  }, [mdRelPath, parseProgress.status]);

  const canViewMd = mdExists || parseProgress.status === "done";

  const navigate = useNavigate();

  const active = useMemo(
    () => subjects.find((s) => s.id === selectedId) ?? null,
    [subjects, selectedId],
  );

  // Auto-select first subject on load
  useEffect(() => {
    if (!loading && selectedId == null && subjects.length > 0) {
      const first = subjects.find((s) => s.is_current) ?? subjects[0];
      setSelectedId(first.id);
    }
  }, [loading, subjects, selectedId]);

  // Auto-open first document when files load
  useEffect(() => {
    if (files.length > 0 && !activeFile) {
      const firstDoc =
        byCategory.home[0] ??
        byCategory.module[0] ??
        byCategory.syllabus[0] ??
        files[0];
      if (firstDoc) openFile(firstDoc);
    }
  }, [files, activeFile, openFile, byCategory]);

  // Reset expanded sections + view mode on subject/file change
  useEffect(() => {
    setImagesExpanded(false);
    setAnnouncementsExpanded(false);
    setFilesExpanded(false);
    setPagesExpanded(false);
  }, [selectedId]);

  useEffect(() => {
    setViewMode("pdf");
  }, [activeFile?.id]);

  // Listen for scrape-file events (trigged by rescrape)
  useEffect(() => {
    const unsub = listen<{
      subject_id: number;
      relative_path: string;
      size_bytes: number;
      category: string | null;
      canvas_id: number | null;
    }>("scrape-file", async (e) => {
      const { subject_id, relative_path, size_bytes, category, canvas_id } = e.payload;
      const filename = relative_path.split("/").pop() ?? relative_path;
      const ext = filename.includes(".") ? filename.split(".").pop()! : "md";
      try {
        await upsertFile(subject_id, filename, relative_path, ext, size_bytes, category ?? undefined, canvas_id ?? undefined);
      } catch { /* ignore */ }
      if (canvas_id != null) {
        setRescraping((prev) => {
          const s = new Set(prev);
          s.delete(canvas_id);
          return s;
        });
      }
      setSelectedId((cur) => {
        if (cur === subject_id) reloadFiles();
        return cur;
      });
    });
    return () => {
      unsub.then((f) => f());
    };
  }, []);


  const rescrapeFile = useCallback(
    async (file: DbFile) => {
      if (!file.canvas_id || !active) return;
      setRescrapeError(null);
      setRescraping((prev) => new Set(prev).add(file.canvas_id!));
      try {
        await invoke("rescrape_file", {
          subjectId: file.subject_id,
          subjectCode: active.code,
          canvasId: file.canvas_id,
        });
      } catch (err) {
        setRescraping((prev) => {
          const s = new Set(prev);
          s.delete(file.canvas_id!);
          return s;
        });
        setRescrapeError(String(err));
      }
    },
    [active],
  );

  // ── Markdown component overrides ───────────────────────────────────────
  const components = useMemo(() => {
    const baseDir = activeFile
      ? activeFile.relative_path.replace(/[^/]+$/, "")
      : "";
    return {
      ...MD_COMPONENTS,
      a: ({ href, children, ...p }: any) => {
        if (href && /^\.\.\//.test(href)) {
          const rel = href.replace(/^\.\.\//, "");
          const target = files.find((f) => f.relative_path.endsWith(rel));
          if (target) {
            return (
              <button
                className="text-primary hover:underline text-left"
                onClick={() => openFile(target)}
                {...p}
              >
                {children}
              </button>
            );
          }
        }
        return (
          <a href={href} target="_blank" rel="noreferrer" className="text-primary hover:underline" {...p}>
            {children}
          </a>
        );
      },
      img: ({ src, alt, ...p }: any) => {
        let resolved = src || "";
        if (resolved && !/^(https?:|data:|asset:|blob:)/.test(resolved) && dataDir) {
          const abs = `${dataDir}/${baseDir}${resolved}`.replace(/\/{2,}/g, "/");
          resolved = convertFileSrc(abs);
        }
        return (
          <img
            className="max-w-full rounded-lg my-3 border border-border"
            src={resolved}
            alt={alt}
            {...p}
          />
        );
      },
    };
  }, [dataDir, activeFile, files, openFile]);

  // ── Render ─────────────────────────────────────────────────────────────
  return (
    <div className="flex h-full">
      {/* Left: subject list */}
      <div
        className="shrink-0 border-r border-border flex flex-col overflow-hidden transition-none"
        style={{ width: subjectsPanel.collapsed ? 0 : subjectsPanel.width }}
      >
        <div className="px-3.5 h-12 flex items-center justify-between border-b border-border-subtle shrink-0 min-w-0">
          {!subjectsPanel.collapsed && (
            <span className="font-semibold text-[13px] text-foreground truncate">Subjects</span>
          )}
        </div>

        <div className="flex-1 overflow-y-auto py-2">
          {loading && (
            <p className="px-4 py-3 text-xs text-muted-foreground">Loading…</p>
          )}

          {!loading && subjects.length === 0 && (
            <div className="px-4 py-3 space-y-1">
              <p className="text-xs text-muted-foreground">No subjects yet.</p>
              <button onClick={() => navigate("/sync")} className="text-xs text-primary hover:underline">
                Go to Sync →
              </button>
            </div>
          )}

          {current.map((s) => (
            <CourseRow
              key={s.id}
              subject={s}
              selected={s.id === selectedId}
              color={courseColor(s.code)}
              onClick={() => setSelectedId(s.id)}
            />
          ))}

          {past.length > 0 && (
            <div className="mt-2">
              <button
                onClick={() => setPastExpanded((v) => !v)}
                className="w-full flex items-center gap-2 px-4 py-1.5 text-[11px] font-medium text-muted-foreground hover:text-foreground transition-colors"
              >
                {pastExpanded ? <ChevronDownIcon className="size-[11px]" /> : <ChevronRightIcon className="size-[11px]" />}
                Past subjects ({past.length})
              </button>
              {pastExpanded &&
                past.map((s) => (
                  <CourseRow
                    key={s.id}
                    subject={s}
                    selected={s.id === selectedId}
                    color={courseColor(s.code)}
                    onClick={() => setSelectedId(s.id)}
                    dimmed
                  />
                ))}
            </div>
          )}
        </div>
      </div>

      <ResizeHandle onMouseDown={subjectsPanel.onMouseDown} />

      {/* Content area */}
      {active ? (
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Course header */}
          <div className="px-5 h-12 flex items-center gap-3 border-b border-border-subtle shrink-0">
            <div>
              <span className="font-semibold text-[13px] text-foreground">{active.code}</span>
              <span className="text-[13px] text-muted-foreground ml-2">{active.name}</span>
            </div>
            {!active.is_current && (
              <span className="ml-auto text-[10px] uppercase tracking-wide text-muted-foreground font-mono bg-surface-raised px-2 py-0.5 rounded">
                {active.term_name ?? "Past"}
              </span>
            )}
          </div>

          <div className="flex flex-1 overflow-hidden">
            {/* File list sidebar */}
            <div
              className="shrink-0 border-r border-border flex flex-col overflow-hidden"
              style={{ width: filesPanel.collapsed ? 0 : filesPanel.width }}
            >
              <div className="flex-1 overflow-y-auto py-2">
                {filesLoading && (
                  <p className="px-4 py-3 text-xs text-muted-foreground">Loading…</p>
                )}

                {!filesLoading && files.length === 0 && (
                  <div className="px-4 py-6 text-center">
                    <ExclamationTriangleIcon className="size-[24px] text-muted-foreground/40 mx-auto mb-2" />
                    <p className="text-xs text-muted-foreground">No content scraped yet.</p>
                    <button onClick={() => navigate("/sync")} className="text-xs text-primary hover:underline mt-1">
                      Run a sync →
                    </button>
                  </div>
                )}

                {/* Home */}
                {byCategory.home.map((f) => (
                  <FileRow key={f.id} icon={BookOpenIcon} label="Overview" size="" active={activeFile?.id === f.id} onClick={() => openFile(f)} />
                ))}

                {/* Syllabus */}
                {byCategory.syllabus.map((f) => (
                  <FileRow key={f.id} icon={BookOpenIcon} label="Syllabus" size="" active={activeFile?.id === f.id} onClick={() => openFile(f)} />
                ))}

                {/* Modules */}
                {byCategory.module.length > 0 && (
                  <div className="mt-1">
                    <div className="px-4 py-1.5 flex items-center gap-2">
                      <Square3Stack3DIcon className="size-[11px] text-muted-foreground" />
                      <span className="text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">Modules</span>
                    </div>
                    {byCategory.module.map((f) => (
                      <FileRow
                        key={f.id}
                        icon={CodeBracketSquareIcon}
                        label={f.filename.replace(/^\d+-/, "").replace(/\.md$/, "")}
                        size=""
                        active={activeFile?.id === f.id}
                        onClick={() => openFile(f)}
                      />
                    ))}
                  </div>
                )}

                <FileCategorySection
                  label="Downloads"
                  icon={PaperClipIcon}
                  files={byCategory.file}
                  expanded={filesExpanded}
                  onToggle={() => setFilesExpanded((v) => !v)}
                  activeFileId={activeFile?.id ?? null}
                  onOpenFile={openFile}
                  dimmed
                  rightIcon={ArrowTopRightOnSquareIcon}
                  onRescrape={rescrapeFile}
                  rescraping={rescraping}
                  liveStatuses={liveStatuses}
                  showExtBadge
                />

                <FileCategorySection
                  label="Pages"
                  icon={DocumentTextIcon}
                  files={byCategory.page}
                  expanded={pagesExpanded}
                  onToggle={() => setPagesExpanded((v) => !v)}
                  activeFileId={activeFile?.id ?? null}
                  onOpenFile={openFile}
                  dimmed
                  labelFormatter={(fn) => fn.replace(/\.md$/, "")}
                />

                <FileCategorySection
                  label="Announcements"
                  icon={MegaphoneIcon}
                  files={byCategory.announcement}
                  expanded={announcementsExpanded}
                  onToggle={() => setAnnouncementsExpanded((v) => !v)}
                  activeFileId={activeFile?.id ?? null}
                  onOpenFile={openFile}
                  dimmed
                  labelFormatter={(fn) => fn.replace(/\.md$/, "")}
                />

                <FileCategorySection
                  label="Images"
                  icon={PhotoIcon}
                  files={byCategory.image}
                  expanded={imagesExpanded}
                  onToggle={() => setImagesExpanded((v) => !v)}
                  activeFileId={activeFile?.id ?? null}
                  onOpenFile={openFile}
                  dimmed
                />
              </div>

              {rescrapeError && (
                <div className="mx-2 mb-2 px-2.5 py-2 rounded-lg bg-destructive/10 border border-destructive/20 text-[11px] text-destructive leading-snug">
                  {rescrapeError}
                </div>
              )}
            </div>

            <ResizeHandle onMouseDown={filesPanel.onMouseDown} />

            {/* Content viewer */}
            <div className="flex-1 min-w-0 overflow-hidden flex flex-col">
              {!activeFile ? (
                <div className="h-full flex items-center justify-center">
                  <p className="text-sm text-muted-foreground">
                    {files.length > 0 ? "Select a file" : "Nothing to display"}
                  </p>
                </div>
              ) : contentLoading ? (
                <div className="h-full flex items-center justify-center gap-2 text-muted-foreground">
                  <ArrowPathMdIcon className="size-[16px] animate-spin" />
                  <span className="text-sm">Loading…</span>
                </div>
              ) : contentError ? (
                <div className="px-8 py-6">
                  <div className="px-3 py-2.5 rounded-lg bg-destructive/10 border border-destructive/20 text-xs text-destructive">
                    Failed to read file: {contentError}
                  </div>
                </div>
              ) : activeFile.category === "file" && isPdf ? (
                <div className="flex flex-col h-full">
                  {/* Toolbar */}
                  <div className="shrink-0 flex items-center justify-between px-4 h-10 border-b border-border bg-surface">
                    <span className="text-xs text-muted-foreground truncate max-w-xs">{activeFile.filename}</span>
                    <div className="flex items-center gap-2">
                      {/* Parse status indicators */}
                      {parseProgress.status === "error" && (
                        <span className="text-[11px] text-destructive">Conversion failed</span>
                      )}
                      {parseProgress.status === "queued" && (
                        <span className="text-[11px] text-muted-foreground">Quality parse queued</span>
                      )}
                      {parseProgress.status === "running" && (
                        <div className="flex items-center gap-2">
                          {parseProgress.total_pages ? (
                            <>
                              <div className="w-24 h-1.5 rounded-full bg-border overflow-hidden">
                                <div
                                  className="h-full bg-primary transition-all duration-500 rounded-full"
                                  style={{ width: `${parsePct || 5}%` }}
                                />
                              </div>
                              <span className="text-[11px] text-muted-foreground tabular-nums">
                                {parseProgress.pages_done ?? 0}/{parseProgress.total_pages} pages
                              </span>
                            </>
                          ) : (
                            <span className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
                              <ArrowPathIcon className="size-[10px] animate-spin" />
                              Starting quality parse…
                            </span>
                          )}
                        </div>
                      )}
                      {/* PDF / Markdown toggle */}
                      {canViewMd && (
                        <div className="flex items-center rounded-md border border-border overflow-hidden text-[11px]">
                          <button
                            onClick={() => setViewMode("pdf")}
                            className={`flex items-center gap-1 px-2 py-1 transition-colors ${viewMode === "pdf" ? "bg-primary text-primary-foreground" : "hover:bg-surface-raised text-muted-foreground"}`}
                          >
                            <DocumentIcon className="size-[11px]" /> PDF
                          </button>
                          <button
                            onClick={() => setViewMode("markdown")}
                            className={`flex items-center gap-1 px-2 py-1 transition-colors ${viewMode === "markdown" ? "bg-primary text-primary-foreground" : "hover:bg-surface-raised text-muted-foreground"}`}
                          >
                            <DocumentTextIcon className="size-[11px]" /> Markdown
                          </button>
                        </div>
                      )}
                    </div>
                  </div>

                  {/* Content */}
                  {viewMode === "pdf" ? (
                    <PDFViewer src={assetUrl(activeFile.relative_path)} />
                  ) : (
                    <MdFromPath
                      relPath={mdRelPath!}
                      components={components}
                      qualityStatus={parseProgress.status}
                      pagesDone={parseProgress.pages_done}
                      totalPages={parseProgress.total_pages}
                    />
                  )}
                </div>
              ) : activeFile.category === "file" ? (
                <div className="h-full flex flex-col items-center justify-center gap-2 text-muted-foreground">
                  <ArrowTopRightOnSquareLgIcon className="size-[24px] opacity-40" />
                  <p className="text-sm">Opened in system viewer</p>
                  <p className="text-xs opacity-60">{activeFile.filename}</p>
                </div>
              ) : activeFile.category === "image" ? (
                <div className="px-8 py-6">
                  <p className="text-xs text-muted-foreground mb-3">{activeFile.filename}</p>
                  <img
                    src={assetUrl(activeFile.relative_path)}
                    alt={activeFile.filename}
                    className="max-w-full rounded-lg border border-border"
                  />
                </div>
              ) : (
                <div className="flex-1 overflow-y-auto">
                <article className="markdown-body px-8 py-6 max-w-3xl">
                  <ReactMarkdown
                    remarkPlugins={[remarkGfm, remarkMath]}
                    rehypePlugins={[rehypeRaw, rehypeKatex]}
                    components={components}
                  >
                    {content}
                  </ReactMarkdown>
                </article>
                </div>
              )}
            </div>
          </div>
        </div>
      ) : (
        !loading && (
          <div className="flex-1 flex items-center justify-center">
            <p className="text-sm text-muted-foreground">Select a subject</p>
          </div>
        )
      )}
    </div>
  );
}
