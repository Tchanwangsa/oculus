import { useEffect, useMemo, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { ArrowSquareOut, File, FileText } from "@phosphor-icons/react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeRaw from "rehype-raw";
import rehypeKatex from "rehype-katex";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { MD_COMPONENTS, normalizeMath } from "@/components/markdown/MdComponents";
import { PDFViewer } from "@/components/files/PDFViewer";
import { docPdfRelPath, isPdfBacked, parsedMdRelPath } from "@/lib/fileTypes";
import { filePageHref } from "@/lib/openFile";
import { libraryImageSrc, libraryLinkTarget } from "@/lib/libraryLinks";
import { useDataDir } from "@/hooks/useDataDir";
import type { DbFile } from "@/lib/db";
import type { FileLocate } from "@/stores/sidePanelStore";
import { readCourseFile } from "@/lib/courseFiles";
import { copyAsMarkdown, dragAsMarkdown } from "@/lib/selectionMarkdown";
import { LoadingFill } from "@/components/ui/PageParts";

/**
 * PDF ↔ parsed-markdown toggle state, lifted out so the host renders the
 * toggle in its own header. Also the "is there markdown?" probe: `mdChecked`
 * holds both controls back until it answers, or `MarkdownUnavailable` would
 * flash on every parsed file.
 */
export function usePdfMd(file: DbFile | null) {
  const isPdf = file != null && isPdfBacked(file.filename);
  const mdRelPath = file ? parsedMdRelPath(file) : null;
  const [viewMode, setViewMode] = useState<"pdf" | "markdown">("pdf");
  const [mdExists, setMdExists] = useState(false);
  const [mdChecked, setMdChecked] = useState(false);

  useEffect(() => {
    setViewMode("pdf");
    setMdExists(false);
    setMdChecked(false);
    if (!mdRelPath) return;
    let live = true;
    readCourseFile(mdRelPath)
      .then((t) => live && setMdExists(t.length > 0))
      .catch(() => live && setMdExists(false))
      .finally(() => live && setMdChecked(true));
    return () => {
      live = false;
    };
  }, [file?.id, mdRelPath]);

  return { isPdf, mdExists, mdChecked, viewMode, setViewMode };
}

export function PdfMdToggle({
  value,
  onChange,
}: {
  value: "pdf" | "markdown";
  onChange: (v: "pdf" | "markdown") => void;
}) {
  return (
    <ToggleGroup
      type="single"
      value={value}
      /* Radix clears the value on pressing the active item; ignore that. */
      onValueChange={(v) => v && onChange(v as "pdf" | "markdown")}
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
  /** Follows a link inside the markdown to another file (same peek). */
  onOpenFile: (file: DbFile) => void;
  /** From `usePdfMd` — which face of a parsed PDF to show. */
  pdfViewMode?: "pdf" | "markdown";
  /** A cited spot in the PDF (`PDFViewer`). */
  locate?: FileLocate;
}

/**
 * Renders one scraped file: markdown pages/announcements, PDFs, and images.
 * Chrome-free — the host owns the header (title + PdfMdToggle).
 */
export function FileViewer({
  file,
  files,
  onOpenFile,
  pdfViewMode = "pdf",
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

  if (pdfRelPath) {
    return (
      <div className="flex flex-col h-full">
        {pdfViewMode === "markdown" && mdRelPath ? (
          <MdFromPath relPath={mdRelPath} components={components} />
        ) : (
          <PDFViewer src={assetUrl(pdfRelPath)} locate={locate} />
        )}
      </div>
    );
  }

  return <MdFromPath relPath={file.relative_path} components={components} />;
}

/**
 * `MD_COMPONENTS` for a library file: links to files we hold locally (`../`
 * paths and raw Canvas `/files/<id>` / `/pages/<slug>` URLs) open in the same
 * peek, others open externally, and relative images resolve against the
 * file's directory (`app/src/lib/libraryLinks.ts`).
 */
function useLibraryMdComponents(
  file: Pick<DbFile, "relative_path">,
  files: DbFile[],
  onOpenFile: (file: DbFile) => void,
) {
  const dataDir = useDataDir();
  return useMemo(() => {
    return {
      ...MD_COMPONENTS,
      a: ({ href, children, ...p }: any) => {
        const target = href ? libraryLinkTarget(href, files) : undefined;
        if (target) {
          return (
            <Button
              variant="link"
              className="h-auto p-0 text-left text-sm font-normal whitespace-normal"
              data-tab-href={filePageHref(target) ?? undefined}
              onClick={() => onOpenFile(target)}
              {...p}
            >
              {children}
            </Button>
          );
        }
        return (
          <a href={href} target="_blank" rel="noreferrer" className="text-brand hover:underline" {...p}>
            {children}
            <ArrowSquareOut size={12} className="inline shrink-0 ml-0.5 mb-0.5 opacity-60" />
          </a>
        );
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

// ── MdFromPath ────────────────────────────────────────────────────────────────

function MdFromPath({
  relPath, components,
}: {
  relPath: string;
  components: any;
}) {
  const [text, setText] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    setText(null);
    setErr(null);
    readCourseFile(relPath)
      .then(setText)
      .catch((e) => setErr(String(e)));
  }, [relPath]);

  if (err)
    return (
      <div className="px-6 py-5">
        <Alert variant="destructive">
          <AlertDescription className="text-xs">
            Failed to load file: {err}
          </AlertDescription>
        </Alert>
      </div>
    );
  if (text === null)
    return (
      <LoadingFill />
    );
  return (
    <div className="flex-1 overflow-y-auto">
      <article
        className="markdown-body px-6 py-5 max-w-3xl"
        onCopy={copyAsMarkdown}
        onDragStart={dragAsMarkdown}
      >
        <ReactMarkdown
          remarkPlugins={[remarkGfm, remarkMath]}
          rehypePlugins={[rehypeRaw, rehypeKatex]}
          components={components}
        >
          {normalizeMath(text)}
        </ReactMarkdown>
      </article>
    </div>
  );
}
