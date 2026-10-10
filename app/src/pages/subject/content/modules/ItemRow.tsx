import { ArrowSquareOut, FileText, PencilLine, Rocket } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { filePageHref, openFileSmart } from "@/lib/files/openFile";
import { FileRecency } from "@/components/files/FileRecency";
import { fileIconFor } from "@/lib/files/fileTypes";
import { resolveTocHref, type ModuleItem } from "@/lib/pipeline/moduleToc";
import type { DbFile } from "@/lib/db";
import { useVideoDownloads } from "@/stores/lectures/videoDownloadStore";
import type { SubjectRef } from "./types";
import { VideoRow } from "./VideoRow";

function itemIcon(item: ModuleItem, target: DbFile | null) {
  if (item.kind === "quiz") return Rocket;
  if (item.kind === "assignment") return PencilLine;
  if (item.kind === "external") return ArrowSquareOut;
  if (target?.category === "file") return fileIconFor(target.filename);
  if (item.kind === "video") return fileIconFor(item.href ?? ".mp4");
  // Everything else is (or links to) a Canvas page.
  return FileText;
}

export function ItemRow({
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
