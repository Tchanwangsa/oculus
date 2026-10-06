import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import {
  ArrowClockwise,
  ArrowSquareOut,
  CaretDown,
  CaretRight,
  CircleNotch,
  DownloadSimple,
  FileText,
  PencilLine,
  Rocket,
  Stack,
  X,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { SubjectLoading, SubjectPage, SubjectEmpty } from "@/components/subjects/SubjectPage";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { useModuleTocs, type LoadedModule } from "@/hooks/useModuleTocs";
import { useSubject } from "@/layouts/SubjectLayout";
import { filePageHref, openFileSmart } from "@/lib/openFile";
import { FileRecency } from "@/components/files/FileRecency";
import { fileIconFor } from "@/lib/fileTypes";
import { resolveTocHref, type ModuleItem } from "@/lib/moduleToc";
import type { DbFile } from "@/lib/db";
import {
  cancelVideoDownload,
  downloadVideo,
  useVideoDownloads,
  type VideoDownload,
} from "@/stores/videoDownloadStore";

/**
 * The Canvas modules page, rebuilt: one collapsible card per module, its items
 * grouped under the SubHeaders Canvas puts between them. Rows open in the
 * side panel.
 */
export default function SubjectModulesPage() {
  const subject = useSubject();
  const navigate = useNavigate();
  const { files, byCategory, loading: filesLoading } = useSubjectFiles(subject.id);
  const modules = useModuleTocs(byCategory.module, filesLoading);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  const toggle = (relPath: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (!next.delete(relPath)) next.add(relPath);
      return next;
    });

  const allCollapsed = useMemo(
    () => modules != null && modules.length > 0 && collapsed.size === modules.length,
    [collapsed, modules],
  );

  if (modules == null) {
    return <SubjectLoading count={3} rowClassName="h-28" spacing="space-y-3" />;
  }

  if (modules.length === 0) {
    return (
      <SubjectEmpty icon={<Stack size={24} className="text-muted-foreground/40" />} title="No modules scraped yet.">
        <Button
          variant="link"
          className="h-auto p-0 text-xs font-normal"
          onClick={() => navigate("/sync")}
        >
          Run a sync →
        </Button>
      </SubjectEmpty>
    );
  }

  return (
    <SubjectPage>
      <div className="mb-3 flex justify-end">
        <Button
          variant="ghost"
          size="sm"
          className="h-6 px-2 text-[11px] text-muted-foreground"
          onClick={() =>
            setCollapsed(
              allCollapsed ? new Set() : new Set(modules.map((m) => m.relPath)),
            )
          }
        >
          {allCollapsed ? "Expand all" : "Collapse all"}
        </Button>
      </div>

      <div className="space-y-2.5">
        {modules.map((mod) => (
          <ModuleCard
            key={mod.relPath}
            module={mod}
            subject={subject}
            files={files}
            open={!collapsed.has(mod.relPath)}
            onToggle={() => toggle(mod.relPath)}
          />
        ))}
      </div>
    </SubjectPage>
  );
}

// ── Pieces ────────────────────────────────────────────────────────────────────

type SubjectRef = { id: number; code: string };

function ModuleCard({
  module: mod, subject, files, open, onToggle,
}: {
  module: LoadedModule;
  subject: SubjectRef;
  files: DbFile[];
  open: boolean;
  onToggle: () => void;
}) {
  return (
    <div className="rounded-lg border border-border overflow-hidden">
      <button
        onClick={onToggle}
        className="w-full flex items-center gap-2 px-3 py-2.5 bg-surface hover:bg-surface-raised transition-colors text-left"
      >
        {open ? (
          <CaretDown size={11} className="text-muted-foreground shrink-0" />
        ) : (
          <CaretRight size={11} className="text-muted-foreground shrink-0" />
        )}
        <span className="text-[12px] font-semibold text-foreground truncate flex-1">
          {mod.title}
        </span>
      </button>

      {open && (
        <div className="divide-y divide-border-subtle">
          {mod.sections.map((section, i) => (
            <div key={i}>
              {section.heading && (
                <div className="px-3 pt-2.5 pb-1">
                  <span className="font-display text-[11px] font-semibold text-muted-foreground">
                    {section.heading}
                  </span>
                </div>
              )}
              {section.items.map((item, j) => (
                <ItemRow
                  key={j}
                  item={item}
                  subject={subject}
                  files={files}
                  moduleRelPath={mod.relPath}
                />
              ))}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function itemIcon(item: ModuleItem, target: DbFile | null) {
  if (item.kind === "quiz") return Rocket;
  if (item.kind === "assignment") return PencilLine;
  if (item.kind === "external") return ArrowSquareOut;
  if (target?.category === "file") return fileIconFor(target.filename);
  if (item.kind === "video") return fileIconFor(item.href ?? ".mp4");
  // Everything else is (or links to) a Canvas page.
  return FileText;
}

function ItemRow({
  item, subject, files, moduleRelPath,
}: {
  item: ModuleItem;
  subject: SubjectRef;
  files: DbFile[];
  moduleRelPath: string;
}) {
  const videoId = item.kind === "video" ? item.canvasFileId : null;
  const download = useVideoDownloads((s) => (videoId == null ? undefined : s.downloads[videoId]));
  const resolved = item.href ? resolveTocHref(item.href, moduleRelPath) : null;
  const internalPath = resolved?.kind === "internal" ? resolved.path : null;
  const target = internalPath
    ? files.find((f) => f.relative_path === internalPath) ??
      // Module docs written before Office rows kept their original names link
      // to the converted PDF ("deck.pptx.pdf") — resolve those to the row.
      files.find((f) => internalPath === `${f.relative_path}.pdf`) ??
      (download?.status === "done" ? download.file : null)
    : null;
  const Icon = itemIcon(item, target);

  const inner = (
    <>
      <span className="shrink-0" style={{ width: item.indent * 14 }} />
      <Icon size={13} className="shrink-0 opacity-60" />
      <span className="text-[12px] truncate flex-1">{item.title}</span>
      {target && <FileRecency file={target} />}
    </>
  );

  const rowClass =
    "w-full flex items-center gap-2.5 px-3 py-2 text-left transition-colors";

  if (target) {
    return (
      <button
        data-tab-href={filePageHref(target) ?? undefined}
        onClick={() => openFileSmart(target)}
        className={cn(rowClass, "text-foreground hover:bg-surface")}
      >
        {inner}
      </button>
    );
  }

  if (videoId != null) {
    return (
      <VideoRow
        canvasFileId={videoId}
        subject={subject}
        download={download}
        rowClass={rowClass}
      >
        {inner}
      </VideoRow>
    );
  }

  if (resolved?.kind === "external") {
    return (
      <a
        href={resolved.url}
        target="_blank"
        rel="noreferrer"
        className={cn(rowClass, "text-foreground hover:bg-surface")}
      >
        {inner}
      </a>
    );
  }

  // Listed by Canvas but not downloadable; shown so the module isn't silently
  // incomplete.
  return (
    <div className={cn(rowClass, "text-muted-foreground/70 cursor-default")}>
      {inner}
    </div>
  );
}

/**
 * A module video not yet in the library: clicking downloads it (or retries a
 * failure) and opens it once it lands, if this row is still on screen.
 */
function VideoRow({
  canvasFileId, subject, download, rowClass, children,
}: {
  canvasFileId: number;
  subject: SubjectRef;
  download: VideoDownload | undefined;
  rowClass: string;
  children: ReactNode;
}) {
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  const start = async () => {
    const file = await downloadVideo(subject, canvasFileId);
    if (file && mounted.current) openFileSmart(file);
  };

  if (download?.status === "downloading") {
    return (
      <div className={cn(rowClass, "text-foreground")}>
        {children}
        {download.percent == null ? (
          <CircleNotch size={11} className="shrink-0 animate-spin text-brand" />
        ) : (
          <span className="shrink-0 text-[11px] tabular-nums text-brand">
            {download.percent}%
          </span>
        )}
        <button
          aria-label="Cancel download"
          title="Cancel download"
          onClick={() => void cancelVideoDownload(canvasFileId)}
          className="-m-1 shrink-0 rounded p-1 text-muted-foreground/60 transition-colors hover:text-destructive"
        >
          <X size={11} />
        </button>
      </div>
    );
  }

  const failed = download?.status === "error" ? download.error : null;
  return (
    <button
      onClick={() => void start()}
      title={failed ? `Download failed: ${failed}` : "Download video"}
      className={cn(rowClass, "group text-foreground hover:bg-surface")}
    >
      {children}
      {failed ? (
        <span className="flex min-w-0 max-w-[50%] shrink items-center gap-1 text-[11px] text-destructive">
          <span className="truncate">{failed}</span>
          <ArrowClockwise size={11} className="shrink-0" />
        </span>
      ) : (
        <DownloadSimple
          size={12}
          className="shrink-0 text-muted-foreground/50 transition-colors group-hover:text-foreground"
        />
      )}
    </button>
  );
}
