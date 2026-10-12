import { useEffect, useMemo, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { File, FileText } from "@phosphor-icons/react";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { FileChip } from "@/components/markdown/FileChip";
import { ReferenceLink } from "@/components/markdown/ReferenceLink";
import { MD_COMPONENTS } from "@/components/markdown/MdComponents";
import { PDFViewer } from "@/components/files/pdf/PDFViewer";
import { docPdfRelPath, isCodeFile, isPdfBacked, isSheetFile, isVideoFile, parsedMdRelPath } from "@/lib/files/fileTypes";
import { openLibraryPath, type FileLocate } from "@/lib/files/openFile";
import { libraryImageSrc, libraryLinkTarget } from "@/lib/files/libraryLinks";
import { useDataDir } from "@/hooks/backend/useDataDir";
import type { DbFile } from "@/lib/db";
import { courseFileHasContent } from "@/lib/files/courseFiles";
import { FileMarkdown } from "@/components/files/FileMarkdown";
import { createPdfMdLink, type PdfMdLink } from "@/components/files/pdf/pdfMdLink";
import { VideoFileViewer } from "@/components/files/VideoFileViewer";
import { CodeFileViewer } from "@/components/files/CodeFileViewer";
import { EmbedSubjectProvider } from "@/components/markdown/embed/EmbedSubject";
import { useParseStore } from "@/stores/sync/parseStore";

/** Which face of a parsed PDF shows: the PDF or its markdown. */
export type PdfViewMode = "pdf" | "markdown";

/**
 * PDF ↔ parsed-markdown toggle state, lifted out so the host renders the
 * toggle in its own header, and the faces' shared state (`link`: the reading
 * position each restores), fresh per file. Also the "is there markdown?"
 * probe: `mdChecked` holds both controls back until it answers, or
 * `MarkdownUnavailable` would flash on every parsed file. A live parse-status
 * change re-asks, so a parse that lands while the page is open shows its
 * markdown.
 */
export function usePdfMd(file: DbFile | null) {
  const isPdf = file != null && isPdfBacked(file.filename);
  const mdRelPath = file ? parsedMdRelPath(file) : null;
  const [viewMode, setViewMode] = useState<PdfViewMode>("pdf");
  // A new link per file: the deps name the file, which the factory never reads.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const link = useMemo(() => createPdfMdLink(), [file?.id, mdRelPath]);
  const [mdExists, setMdExists] = useState(false);
  const [mdChecked, setMdChecked] = useState(false);
  const parseStatus = useParseStore((s) => (file ? s.statuses[file.relative_path] : undefined));

  useEffect(() => {
    setViewMode("pdf");
    setMdExists(false);
    setMdChecked(false);
  }, [file?.id, mdRelPath]);

  // Separate from the reset, so a re-check keeps the last answer on screen.
  useEffect(() => {
    if (!mdRelPath) return;
    let live = true;
    courseFileHasContent(mdRelPath)
      .then((exists) => live && setMdExists(exists))
      .catch(() => live && setMdExists(false))
      .finally(() => live && setMdChecked(true));
    return () => {
      live = false;
    };
  }, [file?.id, mdRelPath, parseStatus]);

  return { isPdf, mdExists, mdChecked, viewMode, setViewMode, link };
}

export function PdfMdToggle({
  value,
  onChange,
}: {
  value: PdfViewMode;
  onChange: (v: PdfViewMode) => void;
}) {
  return (
    <ToggleGroup
      type="single"
      value={value}
      /* Radix clears the value on pressing the active item; ignore that. */
      onValueChange={(v) => v && onChange(v as PdfViewMode)}
      variant="outline"
      size="sm"
      className="text-[11px] shrink-0"
    >
      <ToggleGroupItem
        value="pdf"
        aria-label="View original PDF"
        className="h-6 gap-1 px-2 data-[state=on]:bg-primary data-[state=on]:text-primary-foreground"
      >
        <File size={11} /> PDF
      </ToggleGroupItem>
      <ToggleGroupItem
        value="markdown"
        aria-label="View parsed markdown"
        className="h-6 gap-1 px-2 data-[state=on]:bg-primary data-[state=on]:text-primary-foreground"
      >
        <FileText size={11} /> Markdown
      </ToggleGroupItem>
    </ToggleGroup>
  );
}

interface FileViewerProps {
  file: DbFile;
  /** All of the subject's files — used to resolve in-markdown `../` links. */
  files: DbFile[];
  /** Follows a link inside the markdown to another file. */
  onOpenFile: (file: DbFile) => void;
  /** From `usePdfMd` — which face of a parsed PDF to show. */
  pdfViewMode?: PdfViewMode;
  /** From `usePdfMd` — what the faces share. */
  pdfLink?: PdfMdLink;
  /** A cited spot in the PDF (`PDFViewer`). */
  locate?: FileLocate;
}

/**
 * Renders one scraped file: markdown pages/announcements, PDFs, spreadsheets
 * (as their text), images, videos (in the media player, `VideoFileViewer`)
 * and source files (read-only in `CodeEditor`, `CodeFileViewer`).
 * Chrome-free — the host owns the header (title + PdfMdToggle).
 */
export function FileViewer({
  file,
  files,
  onOpenFile,
  pdfViewMode = "pdf",
  pdfLink,
  locate,
}: FileViewerProps) {
  const dataDir = useDataDir();
  // Office documents render as their converted sibling PDF.
  const pdfRelPath = docPdfRelPath(file);
  const mdRelPath = parsedMdRelPath(file);

  const assetUrl = (relativePath: string) => {
    if (!dataDir) return "";
    return convertFileSrc(`${dataDir}/${relativePath}`.replace(/\/{2,}/g, "/"));
  };

  const components = useLibraryMdComponents(file, files, onOpenFile);
  // The subject lets an Echo360 embed in the page resolve and download.
  const markdown = (relPath: string, link?: PdfMdLink) => (
    <EmbedSubjectProvider subjectId={file.subject_id}>
      <FileMarkdown relPath={relPath} components={components} link={link} />
    </EmbedSubjectProvider>
  );

  if (file.category === "image") {
    return (
      <div className="flex-1 overflow-y-auto px-6 py-5">
        <img
          src={assetUrl(file.relative_path)}
          alt={file.filename}
          className="max-w-full rounded-lg border border-border"
        />
      </div>
    );
  }

  if (isVideoFile(file.filename)) return <VideoFileViewer file={file} />;

  if (isCodeFile(file.filename)) return <CodeFileViewer file={file} />;

  // A spreadsheet is shown as the text Rust converted it to, a table per sheet.
  if (isSheetFile(file.filename) && mdRelPath) {
    return markdown(mdRelPath);
  }

  if (pdfRelPath) {
    return (
      <div className="flex flex-col h-full">
        {pdfViewMode === "markdown" && mdRelPath ? (
          markdown(mdRelPath, pdfLink)
        ) : (
          <PDFViewer
            path={pdfRelPath}
            locate={locate}
            markdownPath={mdRelPath ?? undefined}
            link={mdRelPath ? pdfLink : undefined}
          />
        )}
      </div>
    );
  }

  return markdown(file.relative_path);
}

/**
 * `MD_COMPONENTS` for a library file: links to files we hold locally (`../`
 * paths, paths below the file's folder, and raw Canvas `/files/<id>` /
 * `/pages/<slug>` URLs) go to
 * `onOpenFile`, others open externally, and relative images resolve against the
 * file's directory (`app/src/lib/files/libraryLinks.ts`). Also draws a note's
 * saved versions (`HistoryPanel`).
 */
export function useLibraryMdComponents(
  file: Pick<DbFile, "relative_path">,
  files: DbFile[],
  onOpenFile: (file: DbFile) => void,
) {
  const dataDir = useDataDir();
  return useMemo(() => {
    return {
      ...MD_COMPONENTS,
      a: ({ href, children }: any) => {
        const target = href ? libraryLinkTarget(href, files, file.relative_path) : undefined;
        if (target) {
          return (
            <FileChip path={target.relative_path} onClick={(newTab) => {
              if (newTab) openLibraryPath(target.relative_path, true);
              else onOpenFile(target);
            }}>
              {children}
            </FileChip>
          );
        }
        return <ReferenceLink href={href}>{children}</ReferenceLink>;
      },
      img: ({ src, alt, ...p }: any) => {
        const resolved = libraryImageSrc(src || "", file.relative_path, dataDir);
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
  }, [dataDir, file.relative_path, files, onOpenFile]);
}
