import {
  BookOpen,
  FileText,
  FileWarning,
  ChevronRight,
  ChevronDown,
  Loader2,
  Image as ImageIcon,
  Megaphone,
  Paperclip,
  ExternalLink,
  Layers,
  FileCode,
} from "lucide-react";
import { useState, useEffect, useCallback, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeRaw from "rehype-raw";
import { upsertFile, type DbFile } from "@/lib/db";
import { useDataDir } from "@/hooks/useDataDir";
import { useSubjects } from "@/hooks/useSubjects";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { useFileContent } from "@/hooks/useFileContent";
import { MD_COMPONENTS } from "@/components/markdown/MdComponents";
import { CourseRow, courseColor } from "@/components/subjects/CourseRow";
import { FileCategorySection } from "@/components/files/FileCategorySection";
import { FileRow } from "@/components/files/FileRow";

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

  // Reset expanded sections on subject change
  useEffect(() => {
    setImagesExpanded(false);
    setAnnouncementsExpanded(false);
    setFilesExpanded(false);
    setPagesExpanded(false);
  }, [selectedId]);

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
      <div className="w-56 shrink-0 border-r border-border flex flex-col overflow-hidden">
        <div className="px-4 h-14 flex items-center border-b border-border shrink-0">
          <span className="font-semibold text-sm text-foreground">Subjects</span>
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
                {pastExpanded ? <ChevronDown size={11} /> : <ChevronRight size={11} />}
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

      {/* Content area */}
      {active ? (
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Course header */}
          <div className="px-6 h-14 flex items-center gap-3 border-b border-border shrink-0">
            <BookOpen size={16} className="text-primary" />
            <div>
              <span className="font-semibold text-sm text-foreground">{active.code}</span>
              <span className="text-sm text-muted-foreground ml-2">{active.name}</span>
            </div>
            {!active.is_current && (
              <span className="ml-auto text-[10px] uppercase tracking-wide text-muted-foreground font-mono bg-surface-raised px-2 py-0.5 rounded">
                {active.term_name ?? "Past"}
              </span>
            )}
          </div>

          <div className="flex flex-1 overflow-hidden">
            {/* File list sidebar */}
            <div className="w-60 shrink-0 border-r border-border flex flex-col overflow-hidden">
              <div className="flex-1 overflow-y-auto py-2">
                {filesLoading && (
                  <p className="px-4 py-3 text-xs text-muted-foreground">Loading…</p>
                )}

                {!filesLoading && files.length === 0 && (
                  <div className="px-4 py-6 text-center">
                    <FileWarning size={24} className="text-muted-foreground/40 mx-auto mb-2" />
                    <p className="text-xs text-muted-foreground">No content scraped yet.</p>
                    <button onClick={() => navigate("/sync")} className="text-xs text-primary hover:underline mt-1">
                      Run a sync →
                    </button>
                  </div>
                )}

                {/* Home */}
                {byCategory.home.map((f) => (
                  <FileRow key={f.id} icon={BookOpen} label="Overview" size="" active={activeFile?.id === f.id} onClick={() => openFile(f)} />
                ))}

                {/* Syllabus */}
                {byCategory.syllabus.map((f) => (
                  <FileRow key={f.id} icon={BookOpen} label="Syllabus" size="" active={activeFile?.id === f.id} onClick={() => openFile(f)} />
                ))}

                {/* Modules */}
                {byCategory.module.length > 0 && (
                  <div className="mt-1">
                    <div className="px-4 py-1.5 flex items-center gap-2">
                      <Layers size={11} className="text-muted-foreground" />
                      <span className="text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">Modules</span>
                    </div>
                    {byCategory.module.map((f) => (
                      <FileRow
                        key={f.id}
                        icon={FileCode}
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
                  icon={Paperclip}
                  files={byCategory.file}
                  expanded={filesExpanded}
                  onToggle={() => setFilesExpanded((v) => !v)}
                  activeFileId={activeFile?.id ?? null}
                  onOpenFile={openFile}
                  dimmed
                  rightIcon={ExternalLink}
                  onRescrape={rescrapeFile}
                  rescraping={rescraping}
                />

                <FileCategorySection
                  label="Pages"
                  icon={FileText}
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
                  icon={Megaphone}
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
                  icon={ImageIcon}
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

            {/* Content viewer */}
            <div className="flex-1 overflow-y-auto">
              {!activeFile ? (
                <div className="h-full flex items-center justify-center">
                  <p className="text-sm text-muted-foreground">
                    {files.length > 0 ? "Select a file" : "Nothing to display"}
                  </p>
                </div>
              ) : contentLoading ? (
                <div className="h-full flex items-center justify-center gap-2 text-muted-foreground">
                  <Loader2 size={16} className="animate-spin" />
                  <span className="text-sm">Loading…</span>
                </div>
              ) : contentError ? (
                <div className="px-8 py-6">
                  <div className="px-3 py-2.5 rounded-lg bg-destructive/10 border border-destructive/20 text-xs text-destructive">
                    Failed to read file: {contentError}
                  </div>
                </div>
              ) : activeFile.category === "file" ? (
                <div className="h-full flex flex-col items-center justify-center gap-2 text-muted-foreground">
                  <ExternalLink size={24} className="opacity-40" />
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
                <article className="markdown-body px-8 py-6 max-w-3xl">
                  <ReactMarkdown
                    remarkPlugins={[remarkGfm]}
                    rehypePlugins={[rehypeRaw]}
                    components={components}
                  >
                    {content}
                  </ReactMarkdown>
                </article>
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
