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
  RefreshCw,
} from "lucide-react";
import { useState, useEffect, useCallback, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { appDataDir } from "@tauri-apps/api/path";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeRaw from "rehype-raw";
import { cn } from "@/lib/utils";
import {
  getSubjects,
  getFilesForSubject,
  type Subject,
  type DbFile,
} from "@/lib/db";

// ── Colours ───────────────────────────────────────────────────────────────────

const PALETTE = [
  "#5da0ff",
  "#73c6c2",
  "#a78bfa",
  "#f97316",
  "#22c55e",
  "#ec4899",
  "#eab308",
];

function courseColor(code: string): string {
  let h = 0;
  for (let i = 0; i < code.length; i++) h = code.charCodeAt(i) + ((h << 5) - h);
  return PALETTE[Math.abs(h) % PALETTE.length];
}

function fmtSize(bytes: number | null): string {
  if (bytes == null) return "";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

// ── Component ─────────────────────────────────────────────────────────────────

export default function SubjectsPage() {
  const [subjects, setSubjects] = useState<Subject[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [pastExpanded, setPastExpanded] = useState(false);

  const [files, setFiles] = useState<DbFile[]>([]);
  const [filesLoading, setFilesLoading] = useState(false);
  const [activeFile, setActiveFile] = useState<DbFile | null>(null);
  const [content, setContent] = useState<string>("");
  const [contentLoading, setContentLoading] = useState(false);
  const [contentError, setContentError] = useState<string | null>(null);
  const [dataDir, setDataDir] = useState<string>("");
  const [imagesExpanded, setImagesExpanded] = useState(false);
  const [announcementsExpanded, setAnnouncementsExpanded] = useState(false);
  const [filesExpanded, setFilesExpanded] = useState(false);
  const [pagesExpanded, setPagesExpanded] = useState(false);
  // canvas_ids of files currently being re-downloaded
  const [rescraping, setRescraping] = useState<Set<number>>(new Set());
  const [rescrapeError, setRescrapeError] = useState<string | null>(null);

  const navigate = useNavigate();

  // Resolve a scraped file's on-disk path to an asset: URL the WebView can load.
  const assetUrl = useCallback(
    (relativePath: string) => {
      if (!dataDir) return "";
      const abs = `${dataDir}/${relativePath}`.replace(/\/{2,}/g, "/");
      return convertFileSrc(abs);
    },
    [dataDir],
  );

  // App data dir (for resolving local image paths to asset URLs)
  useEffect(() => {
    appDataDir()
      .then((d) => setDataDir(d.replace(/\\/g, "/").replace(/\/$/, "")))
      .catch(() => {});
  }, []);

  // Listen for scrape-file events triggered by rescrape_file command.
  // Upserts the file into the DB and refreshes the file list.
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
      // Refresh file list if this subject is currently open
      if (canvas_id != null) {
        setRescraping((prev) => { const s = new Set(prev); s.delete(canvas_id); return s; });
      }
      setSelectedId((cur) => {
        if (cur === subject_id) {
          // Re-trigger file load by toggling selectedId (use functional form to avoid stale closure)
          getFilesForSubject(subject_id).then((rows) => setFiles(rows)).catch(() => {});
        }
        return cur;
      });
    });
    return () => { unsub.then((f) => f()); };
  }, []);

  const openFile = useCallback(async (file: DbFile) => {
    setActiveFile(file);
    setContentError(null);
    if (file.category === "image") {
      setContent("");
      setContentLoading(false);
      return;
    }
    if (file.category === "file") {
      setContent("");
      setContentLoading(false);
      invoke("open_course_file", { relativePath: file.relative_path }).catch((err) => {
        setContentError(String(err));
      });
      return;
    }
    setContentLoading(true);
    try {
      const text = await invoke<string>("read_course_file", { relativePath: file.relative_path });
      setContent(text);
    } catch (err) {
      setContentError(String(err));
      setContent("");
    } finally {
      setContentLoading(false);
    }
  }, []);

  // Markdown renderers — img resolves asset URLs; links intercept local file nav.
  const components = useMemo(() => {
    const baseDir = activeFile
      ? activeFile.relative_path.replace(/[^/]+$/, "")
      : "";
    return {
      ...MD_COMPONENTS,
      a: ({ href, children, ...p }: any) => {
        // Intercept relative links to scraped files (../pages/slug.md, ../files/name.pdf)
        if (href && /^\.\.\//.test(href)) {
          const rel = href.replace(/^\.\.\//, ""); // e.g. "pages/slug.md"
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
        return <a href={href} target="_blank" rel="noreferrer" className="text-primary hover:underline" {...p}>{children}</a>;
      },
      img: ({ src, alt, ...p }: any) => {
        let resolved = src || "";
        if (
          resolved &&
          !/^(https?:|data:|asset:|blob:)/.test(resolved) &&
          dataDir
        ) {
          const abs = `${dataDir}/${baseDir}${resolved}`.replace(
            /\/{2,}/g,
            "/",
          );
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

  // Load subjects on mount
  useEffect(() => {
    getSubjects()
      .then((rows) => {
        setSubjects(rows);
        const first = rows.find((r) => r.is_current) ?? rows[0];
        if (first) setSelectedId(first.id);
      })
      .finally(() => setLoading(false));
  }, []);

  // Load files when subject changes
  useEffect(() => {
    if (selectedId == null) return;
    setFilesLoading(true);
    setActiveFile(null);
    setContent("");
    setContentError(null);
    setImagesExpanded(false);
    setAnnouncementsExpanded(false);
    setFilesExpanded(false);
    setPagesExpanded(false);
    getFilesForSubject(selectedId)
      .then((rows) => {
        setFiles(rows);
        // Auto-open first document (skip images)
        const firstDoc =
          rows.find((r) => r.category === "home") ??
          rows.find((r) => r.category === "module") ??
          rows[0];
        if (firstDoc) openFile(firstDoc);
      })
      .finally(() => setFilesLoading(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId]);

  const current = subjects.filter((s) => s.is_current);
  const past = subjects.filter((s) => !s.is_current);
  const active = subjects.find((s) => s.id === selectedId) ?? null;

  const moduleFiles       = files.filter((f) => f.category === "module");
  const pageFiles         = files.filter((f) => f.category === "page");
  const homeFiles         = files.filter((f) => f.category === "home");
  const courseFiles       = files.filter((f) => f.category === "file");
  const announcementFiles = files.filter((f) => f.category === "announcement");
  const imageFiles        = files.filter((f) => f.category === "image");

  const rescrapeFile = useCallback(async (file: DbFile) => {
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
      setRescraping((prev) => { const s = new Set(prev); s.delete(file.canvas_id!); return s; });
      setRescrapeError(String(err));
    }
  }, [active]);

  function selectSubject(s: Subject) {
    setSelectedId(s.id);
  }

  return (
    <div className="flex h-full">
      {/* ── Left: subject list ─────────────────────────────────────────────── */}
      <div className="w-56 shrink-0 border-r border-border flex flex-col overflow-hidden">
        <div className="px-4 h-14 flex items-center border-b border-border shrink-0">
          <span className="font-semibold text-sm text-foreground">
            Subjects
          </span>
        </div>

        <div className="flex-1 overflow-y-auto py-2">
          {loading && (
            <p className="px-4 py-3 text-xs text-muted-foreground">Loading…</p>
          )}

          {!loading && subjects.length === 0 && (
            <div className="px-4 py-3 space-y-1">
              <p className="text-xs text-muted-foreground">No subjects yet.</p>
              <button
                onClick={() => navigate("/sync")}
                className="text-xs text-primary hover:underline"
              >
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
              onClick={() => selectSubject(s)}
            />
          ))}

          {past.length > 0 && (
            <div className="mt-2">
              <button
                onClick={() => setPastExpanded((v) => !v)}
                className="w-full flex items-center gap-2 px-4 py-1.5 text-[11px] font-medium text-muted-foreground hover:text-foreground transition-colors"
              >
                {pastExpanded ? (
                  <ChevronDown size={11} />
                ) : (
                  <ChevronRight size={11} />
                )}
                Past subjects ({past.length})
              </button>
              {pastExpanded &&
                past.map((s) => (
                  <CourseRow
                    key={s.id}
                    subject={s}
                    selected={s.id === selectedId}
                    color={courseColor(s.code)}
                    onClick={() => selectSubject(s)}
                    dimmed
                  />
                ))}
            </div>
          )}
        </div>
      </div>

      {/* ── Middle + right ─────────────────────────────────────────────────── */}
      {active ? (
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Course header */}
          <div className="px-6 h-14 flex items-center gap-3 border-b border-border shrink-0">
            <BookOpen size={16} className="text-primary" />
            <div>
              <span className="font-semibold text-sm text-foreground">
                {active.code}
              </span>
              <span className="text-sm text-muted-foreground ml-2">
                {active.name}
              </span>
            </div>
            {!active.is_current && (
              <span className="ml-auto text-[10px] uppercase tracking-wide text-muted-foreground font-mono bg-surface-raised px-2 py-0.5 rounded">
                {active.term_name ?? "Past"}
              </span>
            )}
          </div>

          <div className="flex flex-1 overflow-hidden">
            {/* File list */}
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
                {homeFiles.map((f) => (
                  <FileRow key={f.id} icon={BookOpen} label="Overview" size="" active={activeFile?.id === f.id} onClick={() => openFile(f)} />
                ))}

                {/* Modules — primary nav, always visible */}
                {moduleFiles.length > 0 && (
                  <div className="mt-1">
                    <div className="px-4 py-1.5 flex items-center gap-2">
                      <Layers size={11} className="text-muted-foreground" />
                      <span className="text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">Modules</span>
                    </div>
                    {moduleFiles.map((f) => (
                      <FileRow key={f.id} icon={FileCode} label={f.filename.replace(/^\d+-/, "").replace(/\.md$/, "")} size="" active={activeFile?.id === f.id} onClick={() => openFile(f)} />
                    ))}
                  </div>
                )}

                {/* Downloads (PDF/slides) — collapsible */}
                {courseFiles.length > 0 && (
                  <div className="mt-2">
                    <button onClick={() => setFilesExpanded((v) => !v)} className="w-full flex items-center gap-2 px-4 py-1.5 text-[11px] font-medium text-muted-foreground hover:text-foreground transition-colors">
                      {filesExpanded ? <ChevronDown size={11} /> : <ChevronRight size={11} />}
                      <Paperclip size={11} />
                      Downloads ({courseFiles.length})
                    </button>
                    {filesExpanded && courseFiles.map((f) => (
                      <FileRow
                        key={f.id}
                        icon={Paperclip}
                        label={f.filename}
                        size={fmtSize(f.size_bytes)}
                        active={activeFile?.id === f.id}
                        onClick={() => openFile(f)}
                        dimmed
                        rightIcon={ExternalLink}
                        onRescrape={f.canvas_id != null ? () => rescrapeFile(f) : undefined}
                        isRescaping={f.canvas_id != null && rescraping.has(f.canvas_id)}
                      />
                    ))}
                  </div>
                )}

                {/* Pages — collapsible */}
                {pageFiles.length > 0 && (
                  <div className="mt-2">
                    <button onClick={() => setPagesExpanded((v) => !v)} className="w-full flex items-center gap-2 px-4 py-1.5 text-[11px] font-medium text-muted-foreground hover:text-foreground transition-colors">
                      {pagesExpanded ? <ChevronDown size={11} /> : <ChevronRight size={11} />}
                      <FileText size={11} />
                      Pages ({pageFiles.length})
                    </button>
                    {pagesExpanded && pageFiles.map((f) => (
                      <FileRow key={f.id} icon={FileText} label={f.filename.replace(/\.md$/, "")} size="" active={activeFile?.id === f.id} onClick={() => openFile(f)} dimmed />
                    ))}
                  </div>
                )}

                {/* Announcements — collapsible */}
                {announcementFiles.length > 0 && (
                  <div className="mt-2">
                    <button onClick={() => setAnnouncementsExpanded((v) => !v)} className="w-full flex items-center gap-2 px-4 py-1.5 text-[11px] font-medium text-muted-foreground hover:text-foreground transition-colors">
                      {announcementsExpanded ? <ChevronDown size={11} /> : <ChevronRight size={11} />}
                      <Megaphone size={11} />
                      Announcements ({announcementFiles.length})
                    </button>
                    {announcementsExpanded && announcementFiles.map((f) => (
                      <FileRow key={f.id} icon={Megaphone} label={f.filename.replace(/\.md$/, "")} size="" active={activeFile?.id === f.id} onClick={() => openFile(f)} dimmed />
                    ))}
                  </div>
                )}

                {/* Images — collapsible */}
                {imageFiles.length > 0 && (
                  <div className="mt-2">
                    <button onClick={() => setImagesExpanded((v) => !v)} className="w-full flex items-center gap-2 px-4 py-1.5 text-[11px] font-medium text-muted-foreground hover:text-foreground transition-colors">
                      {imagesExpanded ? <ChevronDown size={11} /> : <ChevronRight size={11} />}
                      <ImageIcon size={11} />
                      Images ({imageFiles.length})
                    </button>
                    {imagesExpanded && imageFiles.map((f) => (
                      <FileRow key={f.id} icon={ImageIcon} label={f.filename} size={fmtSize(f.size_bytes)} active={activeFile?.id === f.id} onClick={() => openFile(f)} dimmed />
                    ))}
                  </div>
                )}
              </div>

              {rescrapeError && (
                <div className="mx-2 mb-2 px-2.5 py-2 rounded-lg bg-destructive/10 border border-destructive/20 text-[11px] text-destructive leading-snug">
                  {rescrapeError}
                </div>
              )}
            </div>

            {/* Markdown viewer */}
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
                  <p className="text-xs text-muted-foreground mb-3">
                    {activeFile.filename}
                  </p>
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

// ── Sub-components ────────────────────────────────────────────────────────────

function FileRow({
  icon: Icon,
  label,
  size,
  active,
  onClick,
  dimmed = false,
  rightIcon: RightIcon,
  onRescrape,
  isRescaping = false,
}: {
  icon: typeof FileText;
  label: string;
  size: string;
  active: boolean;
  onClick: () => void;
  dimmed?: boolean;
  rightIcon?: typeof FileText;
  onRescrape?: () => void;
  isRescaping?: boolean;
}) {
  return (
    <div
      onClick={onClick}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === "Enter" && onClick()}
      className={cn(
        "w-full text-left px-3 py-2 mx-1 rounded-lg flex items-center gap-2.5 transition-colors cursor-pointer group",
        active
          ? "bg-surface-raised text-foreground"
          : "text-muted-foreground hover:bg-surface hover:text-foreground",
        dimmed && !active && "opacity-70",
      )}
      style={{ width: "calc(100% - 8px)" }}
    >
      <Icon size={13} className="shrink-0" />
      <span className="text-xs flex-1 truncate">{label}</span>
      <span className="text-[10px] text-muted-foreground/70">{size}</span>
      {onRescrape ? (
        <button
          title={isRescaping ? "Re-downloading…" : "Re-download file"}
          disabled={isRescaping}
          onClick={(e) => { e.stopPropagation(); onRescrape(); }}
          className={cn(
            "shrink-0 p-0.5 rounded transition-opacity",
            isRescaping
              ? "opacity-60"
              : "opacity-0 group-hover:opacity-70 hover:!opacity-100 hover:text-foreground",
          )}
        >
          <RefreshCw size={10} className={cn(isRescaping && "animate-spin")} />
        </button>
      ) : RightIcon ? (
        <RightIcon size={10} className="shrink-0 opacity-50" />
      ) : null}
    </div>
  );
}

function CourseRow({
  subject,
  selected,
  color,
  onClick,
  dimmed = false,
}: {
  subject: Subject;
  selected: boolean;
  color: string;
  onClick: () => void;
  dimmed?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "w-full text-left px-3 py-2.5 mx-1 rounded-lg flex items-center gap-3 transition-colors",
        selected
          ? "bg-surface-raised text-foreground"
          : "text-muted-foreground hover:bg-surface hover:text-foreground",
        dimmed && !selected && "opacity-60",
      )}
      style={{ width: "calc(100% - 8px)" }}
    >
      <div
        className="w-2 h-2 rounded-full shrink-0"
        style={{ backgroundColor: color }}
      />
      <div className="min-w-0">
        <p className="text-xs font-semibold truncate">{subject.code}</p>
        <p className="text-[11px] text-muted-foreground truncate">
          {subject.name}
        </p>
      </div>
    </button>
  );
}

// ── Markdown element styling (Tailwind, no typography plugin) ──────────────────

const MD_COMPONENTS = {
  h1: (p: any) => (
    <h1
      className="text-2xl font-bold text-foreground mt-6 mb-3 first:mt-0"
      {...p}
    />
  ),
  h2: (p: any) => (
    <h2
      className="text-xl font-semibold text-foreground mt-6 mb-2.5 pb-1.5 border-b border-border"
      {...p}
    />
  ),
  h3: (p: any) => (
    <h3 className="text-base font-semibold text-foreground mt-5 mb-2" {...p} />
  ),
  h4: (p: any) => (
    <h4 className="text-sm font-semibold text-foreground mt-4 mb-2" {...p} />
  ),
  p: (p: any) => (
    <p className="text-sm text-foreground/90 leading-relaxed my-3" {...p} />
  ),
  a: (p: any) => (
    <a
      className="text-primary hover:underline"
      target="_blank"
      rel="noreferrer"
      {...p}
    />
  ),
  ul: (p: any) => (
    <ul
      className="list-disc pl-5 my-3 space-y-1 text-sm text-foreground/90"
      {...p}
    />
  ),
  ol: (p: any) => (
    <ol
      className="list-decimal pl-5 my-3 space-y-1 text-sm text-foreground/90"
      {...p}
    />
  ),
  li: (p: any) => <li className="leading-relaxed" {...p} />,
  blockquote: (p: any) => (
    <blockquote
      className="border-l-2 border-primary/40 pl-4 my-3 text-sm text-muted-foreground italic"
      {...p}
    />
  ),
  code: ({ className, children, ...p }: any) => {
    // react-markdown v10 dropped `inline` — detect block by fence lang class or newline.
    const isBlock =
      /language-/.test(className ?? "") || String(children).includes("\n");
    return isBlock ? (
      <code
        className="block p-3 rounded-lg bg-surface-raised text-[13px] font-mono text-foreground overflow-x-auto"
        {...p}
      >
        {children}
      </code>
    ) : (
      <code
        className="px-1.5 py-0.5 rounded bg-surface-raised text-[13px] font-mono text-foreground"
        {...p}
      >
        {children}
      </code>
    );
  },
  pre: (p: any) => <pre className="my-3" {...p} />,
  hr: (p: any) => <hr className="my-5 border-border" {...p} />,
  img: (p: any) => (
    <img className="max-w-full rounded-lg my-3 border border-border" {...p} />
  ),
  table: (p: any) => (
    <div className="overflow-x-auto my-3">
      <table className="w-full text-sm border-collapse" {...p} />
    </div>
  ),
  th: (p: any) => (
    <th
      className="border border-border px-3 py-1.5 bg-surface-raised text-left font-semibold text-xs"
      {...p}
    />
  ),
  td: (p: any) => (
    <td
      className="border border-border px-3 py-1.5 text-foreground/90"
      {...p}
    />
  ),
};
