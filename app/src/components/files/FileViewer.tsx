import { useEffect, useMemo, useState } from "react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { ArrowSquareOut, CircleNotch, File, FileText } from "@phosphor-icons/react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeRaw from "rehype-raw";
import rehypeKatex from "rehype-katex";
import "katex/dist/katex.min.css";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { MD_COMPONENTS } from "@/components/markdown/MdComponents";
import { PDFViewer } from "@/components/files/PDFViewer";
import { docPdfRelPath, isPdfBacked, parsedMdRelPath } from "@/lib/fileTypes";
import { useDataDir } from "@/hooks/useDataDir";
import type { DbFile } from "@/lib/db";

/**
 * Shared PDF ↔ parsed-markdown toggle state, lifted out of the viewer so the
 * host (peek header, full-page header) can render the toggle in ITS header
 * instead of stacking a second toolbar.
 */
export function usePdfMd(file: DbFile | null) {
  const isPdf = file != null && isPdfBacked(file.filename);
  const mdRelPath = file ? parsedMdRelPath(file) : null;
  const [viewMode, setViewMode] = useState<"pdf" | "markdown">("pdf");
  const [mdExists, setMdExists] = useState(false);

  useEffect(() => {
    setViewMode("pdf");
    setMdExists(false);
    if (!mdRelPath) return;
    invoke<string>("read_course_file", { relativePath: mdRelPath })
      .then((t) => setMdExists(t.length > 0))
      .catch(() => setMdExists(false));
  }, [file?.id, mdRelPath]);

  return { isPdf, mdExists, viewMode, setViewMode };
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
      /* Radix clears the value when you press the active item; this view
         always needs one of the two. */
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
}: FileViewerProps) {
  const dataDir = useDataDir();
  // Office documents render as their converted sibling PDF.
  const pdfRelPath = docPdfRelPath(file);
  const mdRelPath = parsedMdRelPath(file);

  const assetUrl = (relativePath: string) => {
    if (!dataDir) return "";
    return convertFileSrc(`${dataDir}/${relativePath}`.replace(/\/{2,}/g, "/"));
  };

  // Markdown overrides: links whose target we hold locally open in the same
  // peek — `../`-relative ones (rewritten at scrape time) and raw Canvas
  // `/courses/…/files/<id>` / `/pages/<slug>` URLs (assignment and
  // announcement bodies keep those) — everything else opens externally with
  // an explicit marker. Relative images resolve against the file's directory.
  const components = useMemo(() => {
    const baseDir = file.relative_path.replace(/[^/]+$/, "");
    const localTarget = (href: string): DbFile | undefined => {
      if (/^\.\.\//.test(href)) {
        const rel = href.replace(/^\.\.\//, "");
        return files.find((f) => f.relative_path.endsWith(rel));
      }
      if (href.includes("/courses/")) {
        const fileId = /\/files\/(\d+)/.exec(href)?.[1];
        if (fileId) return files.find((f) => f.canvas_id === Number(fileId));
        const slug = /\/pages\/([^/?#]+)/.exec(href)?.[1];
        if (slug) {
          return (
            files.find((f) => f.relative_path.endsWith(`pages/${slug}.md`)) ??
            // A renamed page keeps its Canvas URL slug while the saved file
            // tracks the title — source_url holds the canonical URL.
            files.find((f) => f.source_url?.endsWith(`/pages/${slug}`))
          );
        }
      }
      return undefined;
    };
    return {
      ...MD_COMPONENTS,
      a: ({ href, children, ...p }: any) => {
        const target = href ? localTarget(href) : undefined;
        if (target) {
          return (
            <Button
              variant="link"
              className="h-auto p-0 text-left text-sm font-normal whitespace-normal"
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
  }, [dataDir, file.relative_path, files, onOpenFile]);

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
          <PDFViewer src={assetUrl(pdfRelPath)} />
        )}
      </div>
    );
  }

  return <MdFromPath relPath={file.relative_path} components={components} />;
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
    invoke<string>("read_course_file", { relativePath: relPath })
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
      <div className="h-full flex items-center justify-center gap-2 text-muted-foreground">
        <CircleNotch size={16} className="animate-spin" />
        <span className="text-sm">Loading…</span>
      </div>
    );
  return (
    <div className="flex-1 overflow-y-auto">
      <article className="markdown-body px-6 py-5 max-w-3xl">
        <ReactMarkdown
          remarkPlugins={[remarkGfm, remarkMath]}
          rehypePlugins={[rehypeRaw, rehypeKatex]}
          components={components}
        >
          {text}
        </ReactMarkdown>
      </article>
    </div>
  );
}
