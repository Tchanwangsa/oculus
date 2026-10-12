import type { ReactNode } from "react";
import { filePageHref, openFileSmart } from "@/lib/files/openFile";
import { FileRecency } from "@/components/files/FileRecency";
import { ParseStateBadge } from "@/components/files/ParseState";
import { fileIconFor, isSheetFile, isVideoFile } from "@/lib/files/fileTypes";
import { fmtSize } from "@/lib/format/format";
import type { DbFile } from "@/lib/db";

/**
 * One fetched file in a `ListCard`: icon, name, size, recency and parse state.
 * `mark` sits between the name and the size; give it a fixed width so rows
 * line up and an answer arriving late never shifts the name.
 */
export function DownloadRow({ file, mark }: { file: DbFile; mark?: ReactNode }) {
  const Icon = fileIconFor(file.filename);
  const iconTone = file.category === "document"
    ? "text-reference-page"
    : /\.pdf$/i.test(file.filename)
      ? "text-reference-pdf"
      : /\.(docx?|pptx?)$/i.test(file.filename)
        ? "text-reference-document"
      : isVideoFile(file.filename)
        ? "text-reference-lecture"
        : isSheetFile(file.filename)
          ? "text-reference-lesson"
          : "text-muted-foreground";

  return (
    <div className="flex items-center gap-3 px-3 py-2 hover:bg-surface transition-colors">
      <button
        data-tab-href={filePageHref(file) ?? undefined}
        onClick={() => openFileSmart(file)}
        className="flex items-center gap-3 flex-1 min-w-0 text-left"
      >
        <Icon size={14} className={`shrink-0 ${iconTone}`} />
        <span className="text-[12px] text-foreground truncate flex-1">
          {file.filename}
        </span>
        {mark}
        {/* Fixed-width columns so rows line up. */}
        <span className="shrink-0 w-17 text-right text-[11px] text-muted-foreground tabular-nums">
          {fmtSize(file.size_bytes)}
        </span>
        <span className="shrink-0 w-13 flex items-center justify-end">
          <FileRecency file={file} />
        </span>
      </button>

      {/* Outside the row's button: a failed parse's icon is its own button.
          A blank column means "no parse", never "all is well". */}
      <span className="shrink-0 w-4 flex items-center justify-center">
        <ParseStateBadge file={file} />
      </span>
    </div>
  );
}
