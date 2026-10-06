import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import { isValidElement, memo } from "react";
import { ArrowSquareOut } from "@phosphor-icons/react";
import { CitationCode, CitationLink, linkCitation } from "@/components/markdown/Citation";
import { FileChip } from "@/components/markdown/FileChip";
import { Mermaid } from "@/components/markdown/Mermaid";
import { OutputEmbed, embedKind } from "@/components/markdown/OutputEmbed";
import { attachmentPath, attachmentSrc } from "@/lib/attachments";
import { useDataDir } from "@/hooks/useDataDir";
import { parseCitation, remarkProsePaths } from "@/lib/citations";
import { libraryPath, openLibraryPath } from "@/lib/openFile";
import { cn } from "@/lib/utils";

// Imported here so every markdown renderer gets it; unlayered, so `.katex`
// keeps its own metrics over Tailwind's layers.
import "katex/dist/katex.min.css";

import { visibleLines, type AnsiSpan } from "@/lib/ansi";
import { hasMath, normalizeMath } from "@/lib/mathMarkdown";

/** Code outside markdown. The app's only `font-mono` lives in this file. */
export function CodeText({ className, ...p }: React.ComponentProps<"pre">) {
  return (
    <pre
      data-selectable
      className={cn("whitespace-pre-wrap break-words font-mono text-[12px] leading-[1.5] text-foreground", className)}
      {...p}
    />
  );
}

const AnsiSpans = ({ spans }: { spans: AnsiSpan[] }) =>
  spans.map((s, i) => (
    <span key={i} className={s.className} style={s.style}>
      {s.text}
    </span>
  ));

/** A command's output, its colours kept. */
export function AnsiText({ lines, className }: { lines: string[]; className?: string }) {
  return (
    <CodeText className={className}>
      {visibleLines(lines).map((spans, i) => (
        <div key={i} className="min-h-[1.5em]">
          <AnsiSpans spans={spans} />
        </div>
      ))}
    </CodeText>
  );
}

/** The newest line of a running command in a one-line terminal block. */
export function TerminalLine({ lines, className }: { lines: string[]; className?: string }) {
  const spans = visibleLines(lines).filter((l) => l.length > 0).pop();
  if (!spans) return null;
  return (
    <div
      className={cn(
        "truncate rounded-md border border-border-subtle bg-surface px-2 py-1 font-mono text-[11px] leading-none text-foreground",
        className,
      )}
    >
      <AnsiSpans spans={spans} />
    </div>
  );
}

/** The source of a ```mermaid fence, or `null` for every other `<pre>`. The
 *  trailing newline is dropped — some mermaid grammars read it as an empty
 *  statement. */
function mermaidSource(children: React.ReactNode): string | null {
  const el = Array.isArray(children) ? children.find(isValidElement) : children;
  if (!isValidElement(el)) return null;
  const props = el.props as { className?: string; children?: React.ReactNode };
  if (!/(^|\s)language-mermaid(\s|$)/.test(props.className ?? "")) return null;
  const source = String(props.children ?? "").replace(/\n+$/, "");
  return source.trim() ? source : null;
}

/**
 * A picture in this library. A bare library path as `src` resolves against
 * the dev server's origin and 404s, so it goes through [`attachmentSrc`].
 * A component (not inline in `MD_COMPONENTS`) because it needs a hook.
 */
function LibraryImage({ path, alt }: { path: string; alt?: string }) {
  const dataDir = useDataDir();
  return (
    <img
      src={attachmentSrc(dataDir, path)}
      alt={alt ?? ""}
      title={path}
      onClick={(e) => openLibraryPath(path, e.metaKey || e.ctrlKey)}
      className="my-3 max-h-80 w-auto max-w-full cursor-pointer rounded-lg border border-border"
    />
  );
}

/**
 * `![alt](src)`: a library picture or HTML page drawn in place
 * (`OutputEmbed`), any other library file as its chip, then any src with a
 * scheme as-is; a schemeless path to nothing renders its alt text rather
 * than WebKit's broken-image glyph.
 */
function MdImage({ src, alt, ...p }: any) {
  const raw = typeof src === "string" ? src : "";
  const path = attachmentPath(raw) ?? libraryPath(raw);
  if (path && embedKind(path)) return <OutputEmbed path={path} alt={alt} />;
  if (path) return <FileChip path={path} onClick={(newTab) => openLibraryPath(path, newTab)} />;
  if (/^[a-z][a-z0-9+.-]*:/i.test(raw))
    return (
      <img className="max-w-full max-h-80 rounded-lg my-3 border border-border" src={raw} alt={alt} {...p} />
    );
  return <span title={raw}>{alt}</span>;
}

/**
 * The `code` renderer. `pictures` is off for [`InlineMd`]: `disallowedElements`
 * only filters the parsed tree, not an `<img>` a component draws.
 */
function codeRenderer(pictures: boolean) {
  // An inline span that is nothing but a citation (`lib/citations.ts`) draws
  // as a FileChip, or as the picture for an attachment; anything else stays code.
  return ({ className, children, ...p }: any) => {
    const isBlock = /language-/.test(className ?? "") || String(children).includes("\n");
    if (isBlock)
      return (
        <code className="block p-3 rounded-lg bg-surface-raised text-[13px] font-mono text-foreground overflow-x-auto" {...p}>
          {children}
        </code>
      );
    const span = String(children);
    const att = pictures ? attachmentPath(span) : null;
    if (att) return <LibraryImage path={att} alt="Attached picture" />;
    const code = (
      <code className="px-1.5 py-0.5 rounded bg-surface-raised text-[13px] font-mono text-foreground" {...p}>
        {children}
      </code>
    );
    const shape = parseCitation(span);
    return shape ? <CitationCode shape={shape} code={code} /> : code;
  };
}

export const MD_COMPONENTS: Components = {
  h1: (p: any) => (
    <h1 className="text-2xl font-bold text-foreground mt-6 mb-3 first:mt-0" {...p} />
  ),
  h2: (p: any) => (
    <h2 className="text-xl font-semibold text-foreground mt-6 mb-2.5 pb-1.5 border-b border-border" {...p} />
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
  // Citations open in-app; web URLs stay plain anchors, which `AppLayout`
  // routes to the in-app browser.
  a: ({ href, children, ...p }: any) => {
    const shape = linkCitation(href, children);
    if (shape) {
      return (
        <CitationLink shape={shape} {...p}>
          {children}
        </CitationLink>
      );
    }
    // A schemeless non-library path would resolve against the app's origin
    // and 404, so it stays text.
    if (!/^[a-z][a-z0-9+.-]*:/i.test(href ?? "")) return <span title={href}>{children}</span>;
    return (
      <a href={href} className="text-brand hover:underline" target="_blank" rel="noreferrer" {...p}>
        {children}
        {/^https?:/.test(href ?? "") && (
          <ArrowSquareOut size={12} className="inline shrink-0 ml-0.5 mb-0.5 opacity-60" />
        )}
      </a>
    );
  },
  ul: (p: any) => (
    <ul className="list-disc pl-5 my-3 space-y-1 text-sm text-foreground/90" {...p} />
  ),
  ol: (p: any) => (
    <ol className="list-decimal pl-5 my-3 space-y-1 text-sm text-foreground/90" {...p} />
  ),
  li: (p: any) => <li className="leading-relaxed" {...p} />,
  // Deliberately not italic: long quotes read slowly in italic.
  blockquote: (p: any) => (
    <blockquote className="border-l-2 border-border pl-4 my-3 text-sm text-muted-foreground" {...p} />
  ),
  code: codeRenderer(true),
  // Mermaid is caught at `pre`, not `code`: inside a `<pre>`, `white-space:
  // pre` renders the gaps in mermaid's SVG. `Mermaid` falls back to the `<pre>`.
  pre: ({ children, ...p }: any) => {
    const chart = mermaidSource(children);
    const block = (
      <pre className="my-3" {...p}>
        {children}
      </pre>
    );
    return chart ? <Mermaid code={chart}>{block}</Mermaid> : block;
  },
  hr: (p: any) => <hr className="my-5 border-border" {...p} />,
  img: MdImage,
  table: (p: any) => (
    <div className="overflow-x-auto my-3">
      <table className="w-full text-sm border-collapse" {...p} />
    </div>
  ),
  th: (p: any) => (
    <th className="border border-border px-3 py-1.5 bg-surface-raised text-left font-semibold text-xs" {...p} />
  ),
  td: (p: any) => (
    <td className="border border-border px-3 py-1.5 text-foreground/90" {...p} />
  ),
};

// ── Two ready-made renderers ─────────────────────────────────────────────────

/** Hoisted for stable array identity across re-renders. */
const PLAIN = [remarkGfm, remarkProsePaths];
const WITH_MATH = [remarkGfm, remarkMath, remarkProsePaths];
const KATEX = [rehypeKatex];
const NO_PLUGINS: never[] = [];

/**
 * Markdown with no block elements in the output, for text inside a `<button>`
 * (transcript lines, chapter summaries): a block element inside a button makes
 * WebKit close the button early.
 */
export const InlineMd = memo(function InlineMd({ text, className }: { text: string; className?: string }) {
  const math = hasMath(text);
  return (
    <span className={cn("md-inline", className)}>
      <ReactMarkdown
        remarkPlugins={math ? WITH_MATH : PLAIN}
        rehypePlugins={math ? KATEX : NO_PLUGINS}
        components={INLINE_COMPONENTS}
        disallowedElements={BLOCKS}
        unwrapDisallowed
      >
        {math ? normalizeMath(text) : text}
      </ReactMarkdown>
    </span>
  );
});

/** Unwrapped (`unwrapDisallowed`), not dropped, so their text survives. */
const BLOCKS = ["h1", "h2", "h3", "h4", "h5", "h6", "hr", "img", "table", "blockquote"];

const INLINE_COMPONENTS: Components = {
  ...MD_COMPONENTS,
  code: codeRenderer(false),
  p: (p: any) => <span {...p} />,
  ul: (p: any) => <span {...p} />,
  ol: (p: any) => <span {...p} />,
  li: (p: any) => <span {...p} />,
  pre: (p: any) => <span {...p} />,
};

/**
 * Full markdown at docked-panel size (chat replies). `.md-compact` in
 * `index.css` scales the document-sized components down; it is unlayered so
 * it beats their utilities.
 */
export const CompactMd = memo(function CompactMd({ text, className }: { text: string; className?: string }) {
  const math = hasMath(text);
  return (
    <div data-selectable className={cn("md-compact min-w-0", className)}>
      <ReactMarkdown
        remarkPlugins={math ? WITH_MATH : PLAIN}
        rehypePlugins={math ? KATEX : NO_PLUGINS}
        components={MD_COMPONENTS}
      >
        {math ? normalizeMath(text) : text}
      </ReactMarkdown>
    </div>
  );
});
