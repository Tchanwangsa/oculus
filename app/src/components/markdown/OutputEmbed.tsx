import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { ArrowSquareOut, ArrowsOutSimple, CircleNotch, FileHtml, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogCanvas, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { ImageLightbox } from "@/components/ui/Lightbox";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useDataDir } from "@/hooks/useDataDir";
import { attachmentSrc } from "@/lib/attachments";
import { readCourseFile } from "@/lib/courseFiles";
import { isDark, subscribeDark } from "@/lib/theme";

/**
 * A picture or HTML page from the library, drawn inline in a reply — what an
 * agent made in `agents/` (a chart, a visual summary) shown where it names it.
 *
 * Every element is a `span` or phrasing content: `![alt](path)` alone on a
 * line parses as `<p><img></p>`, and a `<div>` inside a `<p>` is invalid.
 * The wrapper carries `data-md`, so a copied selection gets the markdown
 * back (`lib/selectionMarkdown.ts`), not the asset URL or the header's text.
 */

const IMAGE = /\.(png|jpe?g|gif|webp|avif|svg)$/i;
const HTML = /\.html?$/i;

/** What `OutputEmbed` draws for a path, by extension; `null` for anything else. */
export function embedKind(path: string): "image" | "html" | null {
  const bare = path.split(/[?#]/)[0];
  if (IMAGE.test(bare)) return "image";
  if (HTML.test(bare)) return "html";
  return null;
}

/** `path` is data-dir-relative (`agents/…`, `courses/…`, `lectures/…`). */
export function OutputEmbed({ path, alt }: { path: string; alt?: string }) {
  const kind = embedKind(path);
  if (kind === "image") return <EmbedImage path={path} alt={alt} />;
  if (kind === "html") return <EmbedPage path={path} alt={alt} />;
  return null;
}

const basename = (path: string) => path.slice(path.lastIndexOf("/") + 1);

/** The markdown this embed stands for; a path with a space needs `<…>`. */
function sourceOf(path: string, alt?: string): string {
  const target = /[\s()<>]/.test(path) ? `<${path}>` : path;
  return `![${(alt ?? "").replace(/[[\]]/g, "\\$&")}](${target})`;
}

function openExternally(path: string) {
  invoke("open_course_file", { relativePath: path }).catch(console.error);
}

// ── Picture ──────────────────────────────────────────────────────────────────

function EmbedImage({ path, alt }: { path: string; alt?: string }) {
  const dataDir = useDataDir();
  const src = attachmentSrc(dataDir, path);
  const [broken, setBroken] = useState(false);
  const [open, setOpen] = useState(false);
  const label = alt || basename(path);

  // The alt or filename, never WebKit's broken-image glyph.
  if (broken)
    return (
      <span data-md={sourceOf(path, alt)} title={path} className="text-muted-foreground">
        {label}
      </span>
    );

  return (
    <span data-md={sourceOf(path, alt)} className="my-3 block">
      <button
        type="button"
        onClick={() => setOpen(true)}
        aria-label={`Open ${label}`}
        className="block max-w-full cursor-zoom-in"
      >
        {/* Not until the data dir resolves: an empty `src` fires `error`. */}
        {src && (
          <img
            src={src}
            alt={alt ?? ""}
            title={path}
            onError={() => setBroken(true)}
            className="block max-h-80 w-auto max-w-full rounded-lg border border-border"
          />
        )}
      </button>
      {alt && <span className="mt-1.5 block text-xs text-muted-foreground">{alt}</span>}
      <ImageLightbox src={src} alt={alt} open={open} onOpenChange={setOpen} />
    </span>
  );
}

// ── HTML page ────────────────────────────────────────────────────────────────

/** The iframe's height before the page first reports, and the clamp after;
 *  taller content scrolls inside the frame. */
const DEFAULT_HEIGHT = 320;
const MIN_HEIGHT = 120;
const MAX_HEIGHT = 560;

const HEIGHT_MESSAGE = "oculus-embed-height";

/** Injected into the page: posts its content height to the parent on load and
 *  on every resize. The body's scroll height catches overflow that the root's
 *  box doesn't. */
const REPORT_HEIGHT = `(function(){var last=0;function post(){var d=document.documentElement,b=document.body;var h=Math.ceil(Math.max(d.getBoundingClientRect().height,b?b.scrollHeight:0));if(h!==last){last=h;parent.postMessage({type:"${HEIGHT_MESSAGE}",height:h},"*");}}addEventListener("DOMContentLoaded",post);addEventListener("load",post);if(typeof ResizeObserver!=="undefined")new ResizeObserver(post).observe(document.documentElement);})();`;

/**
 * The page's directory as an asset URL ending in `/`, for `<base href>`.
 * `convertFileSrc` encodes slashes too, so the data dir becomes one segment
 * and the library path keeps real ones — `../` then resolves within the
 * library; the asset protocol percent-decodes the whole path either way.
 */
function baseHref(dataDir: string, path: string): string {
  const dirs = path.split("/").slice(0, -1).filter(Boolean);
  return `${convertFileSrc(dataDir)}/${dirs.map((d) => `${encodeURIComponent(d)}/`).join("")}`;
}

const escapeAttr = (s: string) => s.replace(/&/g, "&amp;").replace(/"/g, "&quot;");

/**
 * The page with a `<base>`, a colour scheme and the height reporter at the
 * top of its `<head>` (one is opened if absent). The scheme is the app's own,
 * not `light dark`, because the app themes by class and the OS may disagree;
 * a page that declares one keeps it.
 */
function prepare(html: string, base: string, dark: boolean): string {
  let tags = "";
  if (!/<base[\s>]/i.test(html)) tags += `<base href="${escapeAttr(base)}">`;
  if (!/<meta[^>]+name\s*=\s*["']?color-scheme|color-scheme\s*:/i.test(html))
    tags += `<meta name="color-scheme" content="${dark ? "dark" : "light"}">`;
  tags += `<script>${REPORT_HEIGHT}</script>`;

  const head = /<head(\s[^>]*)?>/i.exec(html);
  if (head) return splice(html, head.index + head[0].length, tags);
  // After `<html>`, else after a doctype (before it would mean quirks mode).
  const opener = /<html(\s[^>]*)?>/i.exec(html) ?? /<!doctype[^>]*>/i.exec(html);
  return splice(html, opener ? opener.index + opener[0].length : 0, `<head>${tags}</head>`);
}

const splice = (s: string, at: number, insert: string) => s.slice(0, at) + insert + s.slice(at);

function EmbedPage({ path, alt }: { path: string; alt?: string }) {
  const dataDir = useDataDir();
  const dark = useSyncExternalStore(subscribeDark, isDark);
  const [source, setSource] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const [height, setHeight] = useState(DEFAULT_HEIGHT);
  const [open, setOpen] = useState(false);
  const frame = useRef<HTMLIFrameElement>(null);
  const title = alt || basename(path);

  useEffect(() => {
    let live = true;
    setSource(null);
    setFailed(false);
    readCourseFile(path).then(
      (text) => live && setSource(text),
      () => live && setFailed(true),
    );
    return () => {
      live = false;
    };
  }, [path]);

  // A theme flip rebuilds the document (and reloads the frame) so the
  // injected scheme follows; the last height holds until it reports again.
  const doc = useMemo(
    () => (source != null && dataDir ? prepare(source, baseHref(dataDir, path), dark) : null),
    [source, dataDir, path, dark],
  );

  // Only this frame's own reports; the expanded copy and other embeds post too.
  useEffect(() => {
    const onMessage = (e: MessageEvent) => {
      if (!frame.current || e.source !== frame.current.contentWindow) return;
      if (e.data?.type !== HEIGHT_MESSAGE) return;
      const h = Number(e.data.height);
      if (Number.isFinite(h)) setHeight(Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, Math.ceil(h))));
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, []);

  return (
    <span
      data-md={sourceOf(path, alt)}
      className="my-3 block overflow-hidden rounded-lg border border-border bg-card"
    >
      <span className="flex h-8 min-w-0 select-none items-center gap-1.5 border-b border-border pr-1 pl-2.5 text-xs text-muted-foreground">
        <FileHtml size={14} className="shrink-0" />
        <span className="min-w-0 flex-1 truncate" title={path}>
          {title}
        </span>
        <IconAction label="Expand" onClick={() => setOpen(true)} disabled={!doc}>
          <ArrowsOutSimple size={13} />
        </IconAction>
        <IconAction label="Open externally" onClick={() => openExternally(path)}>
          <ArrowSquareOut size={13} />
        </IconAction>
      </span>
      {failed ? (
        <span className="block px-3 py-3 text-xs break-all text-muted-foreground">
          Couldn't read {path}
        </span>
      ) : doc == null ? (
        <span style={{ height }} className="flex items-center justify-center text-muted-foreground">
          <CircleNotch size={14} className="animate-spin" />
        </span>
      ) : (
        <iframe
          ref={frame}
          // Never `allow-same-origin`: the page would get the app's origin and the Tauri IPC.
          sandbox="allow-scripts"
          srcDoc={doc}
          title={title}
          style={{ height }}
          className="block w-full border-0"
        />
      )}
      {doc != null && (
        <PageViewer doc={doc} title={title} path={path} open={open} onOpenChange={setOpen} />
      )}
    </span>
  );
}

/** The same page full-window, at full height — `DialogCanvas` with a
 *  toolbar in `ui/Lightbox.tsx`'s grammar. No zoom: the page lays out to
 *  the window instead. */
function PageViewer({
  doc,
  title,
  path,
  open,
  onOpenChange,
}: {
  doc: string;
  title: string;
  path: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogCanvas>
        <DialogTitle className="sr-only">{title}</DialogTitle>
        <DialogDescription className="sr-only">The page, full-window. Escape to close.</DialogDescription>
        <div className="flex min-h-0 flex-1 p-9 pb-20">
          <iframe
            // Same sandbox as the inline frame, and for the same reason.
            sandbox="allow-scripts"
            srcDoc={doc}
            title={title}
            className="min-h-0 w-full flex-1 rounded-lg border border-border bg-card"
          />
        </div>
        <div className="pointer-events-none absolute inset-x-0 bottom-6 flex justify-center px-6">
          <div className="pointer-events-auto flex min-w-0 items-center gap-0.5 rounded-full border border-border bg-card/90 p-1 pl-3 text-xs text-muted-foreground shadow-md backdrop-blur-sm">
            <span className="max-w-80 min-w-0 truncate" title={path}>
              {title}
            </span>
            <div className="mx-1 h-4 w-px shrink-0 bg-border" />
            <IconAction label="Open externally" onClick={() => openExternally(path)} size="icon-sm">
              <ArrowSquareOut size={14} />
            </IconAction>
            <IconAction label="Close" onClick={() => onOpenChange(false)} size="icon-sm">
              <X size={14} />
            </IconAction>
          </div>
        </div>
      </DialogCanvas>
    </Dialog>
  );
}

function IconAction({
  label,
  onClick,
  disabled,
  size = "icon-xs",
  children,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  size?: "icon-xs" | "icon-sm";
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size={size}
          onClick={onClick}
          disabled={disabled}
          aria-label={label}
          className="shrink-0"
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
