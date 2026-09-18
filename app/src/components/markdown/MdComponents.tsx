import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import { isValidElement } from "react";
import { ArrowSquareOut } from "@phosphor-icons/react";
import { FileChip } from "@/components/markdown/FileChip";
import { Mermaid } from "@/components/markdown/Mermaid";
import { attachmentPath, attachmentSrc } from "@/lib/attachments";
import { useDataDir } from "@/hooks/useDataDir";
import { libraryPath, openLibraryPath } from "@/lib/openFile";
import { cn } from "@/lib/utils";

// KaTeX's own stylesheet, imported here rather than at a call site: this is
// the module every markdown renderer in the app already goes through for its
// components, so the styles arrive with them instead of depending on which
// viewer happens to be in the bundle. Unlayered, so it outranks the Tailwind
// layers — which is what a `.katex` span needs to keep its own metrics.
import "katex/dist/katex.min.css";

/** Whether a reply is worth running the maths plugins over.
 *
 *  KaTeX is the most expensive thing in the markdown pipeline and most text
 *  has no maths in it at all, so both plugins are gated on a delimiter being
 *  present. `\(` and `\[` count even though remark-math cannot read them —
 *  [`normalizeMath`] turns those into the ones it can, and the gate is asked
 *  before that runs. */
export const MATH = /\$|\\\(|\\\[/;

/** Code fences and inline code spans, which are left exactly as written. */
const CODE = /(```[\s\S]*?```|~~~[\s\S]*?~~~|`[^`\n]*`)/g;
const DISPLAY = /\\\[([\s\S]+?)\\\]/g;
const INLINE = /\\\(([\s\S]+?)\\\)/g;

/**
 * Rewrite LaTeX's `\(…\)` and `\[…\]` into the `$…$` and `$$…$$` remark-math
 * actually reads.
 *
 * The pair never survives to the maths plugin on its own: CommonMark treats a
 * backslash before ASCII punctuation as an escape, so the parser eats the
 * backslash and hands on a bare `(` long before remark-math looks for a
 * delimiter — the formula renders as plain text with its parentheses intact,
 * which reads like the model simply chose not to use maths. It is also the
 * form a model reaches for by default, so the prompt asking for dollars
 * (`HARNESS.template.md`) is the fix and this is the net under it.
 *
 * Code is stepped over: `\(` inside a fence is someone's source, not a
 * formula.
 */
export function normalizeMath(text: string): string {
  return text
    .split(CODE)
    .map((part, i) =>
      i % 2 === 1
        ? part
        : part.replace(DISPLAY, (_, m) => `$$${m}$$`).replace(INLINE, (_, m) => `$${m}$`),
    )
    .join("");
}

/**
 * Code outside markdown — a command line, a tool's output. This is the one
 * file that may use `font-mono` (root CLAUDE.md), so anything code-shaped
 * elsewhere in the app comes here for it.
 */
export function CodeText({ className, ...p }: React.ComponentProps<"pre">) {
  return (
    <pre
      className={cn("whitespace-pre-wrap break-words font-mono text-[12px] leading-[1.5] text-foreground", className)}
      {...p}
    />
  );
}

/** The source of a ```mermaid fence, or `null` for every other `<pre>`.
 *
 *  react-markdown hands `pre` a single `code` child carrying the language as a
 *  class, so the fence's info string is only readable from up here. The tail
 *  newline every fence ends with is dropped — mermaid reads it as an empty
 *  final statement in some diagram grammars. */
function mermaidSource(children: React.ReactNode): string | null {
  const el = Array.isArray(children) ? children.find(isValidElement) : children;
  if (!isValidElement(el)) return null;
  const props = el.props as { className?: string; children?: React.ReactNode };
  if (!/(^|\s)language-mermaid(\s|$)/.test(props.className ?? "")) return null;
  const source = String(props.children ?? "").replace(/\n+$/, "");
  return source.trim() ? source : null;
}

/**
 * A picture that lives in this library, drawn from the asset URL the webview
 * can actually load and opening full size on click.
 *
 * A path is not a `src`. `agents/attachments/3.png` or
 * `courses/<CODE>/images/fig.png` handed straight to an `<img>` is resolved
 * against the dev server's origin and 404s — the same dead end `a` refuses to
 * make a link of — so it goes through [`attachmentSrc`], which is the one
 * place that knows the data directory and `convertFileSrc`. The question
 * bubble (`components/harness/Timeline.tsx`) is these same three lines for the
 * picture the student attached; this is the renderer's copy of them, so a
 * picture reads the same whether the student sent it or the agent wrote it.
 *
 * **A real component, not an expression inside `MD_COMPONENTS`**: the data
 * directory arrives from a hook, and that object is a plain map of components
 * with nowhere to call one.
 */
function LibraryImage({ path, alt }: { path: string; alt?: string }) {
  // One IPC call for the whole app (`useDataDir`), so a reply carrying six
  // pictures asks once — and an empty string until it lands, which paints
  // nothing rather than a wrong URL.
  const dataDir = useDataDir();
  return (
    <img
      src={attachmentSrc(dataDir, path)}
      alt={alt ?? ""}
      // The path is what the message actually says, and hovering is the only
      // place it is still reachable by eye — same bargain `FileChip` makes.
      title={path}
      // Opens the way every other library path in a reply does, modifier
      // included: `MD_COMPONENTS.a` carries it by hand for the same reason a
      // chip does — a path is not a route until a row is looked up
      // (`openLibraryPath`).
      onClick={(e) => openLibraryPath(path, e.metaKey || e.ctrlKey)}
      className="my-3 max-h-80 w-auto max-w-full cursor-pointer rounded-lg border border-border"
    />
  );
}

/**
 * `![alt](src)` — a picture when the `src` names something this webview can
 * load, and the alt text when it does not.
 *
 * Three cases, in the order `splitLibraryPaths` uses: an attachment is checked
 * first because it *is* a path in this library, just not one under `courses/`;
 * then a course file; then a src with a scheme, which the webview fetches
 * itself and which is therefore left exactly as it was — `http(s)` from the
 * web, `data:`/`blob:`/`asset:` from inside this app.
 *
 * What is left is a relative path pointing at nothing, and it is the reason
 * this component exists rather than a `<img {...p}>`: a broken link fails
 * *silently* in `a` and *visibly* in an image, parking WebKit's broken-image
 * glyph in the middle of a reply. So it takes the `a` handler's answer — keep
 * the text, keep the src on the title for whoever goes hunting — and the
 * reply reads as prose with a word in it instead of as a failure.
 */
function MdImage({ src, alt, ...p }: any) {
  const raw = typeof src === "string" ? src : "";
  const path = attachmentPath(raw) ?? libraryPath(raw);
  if (path) return <LibraryImage path={path} alt={alt} />;
  if (/^[a-z][a-z0-9+.-]*:/i.test(raw))
    return (
      <img className="max-w-full max-h-80 rounded-lg my-3 border border-border" src={raw} alt={alt} {...p} />
    );
  return <span title={raw}>{alt}</span>;
}

/**
 * The `code` renderer, in two versions.
 *
 * `pictures` is off for [`InlineMd`], whose output goes inside a `<button>`
 * that seeks: it lists `img` in [`BLOCKS`] so the markdown grammar cannot put
 * one there, but `disallowedElements` is applied to the parsed tree and knows
 * nothing about a picture a *component* decides to draw. Turning the fence
 * case off is how that promise keeps holding.
 */
function codeRenderer(pictures: boolean) {
  // A library path the agent quoted back — `` `courses/…/13.pdf` `` — is the
  // same mention the student picked in the composer, so it is drawn as the
  // same chip and opens the same way; an attached picture, written into the
  // message as `` `agents/attachments/…` `` and drawn as the picture in the
  // question bubble, is drawn as the picture here too rather than being the
  // one place it reads as a path. Only a fence that is *nothing but* a path:
  // `oculus read`, a flag or a snippet is code, and code keeps the monospace
  // (root CLAUDE.md) that neither a chip nor a picture has.
  return ({ className, children, ...p }: any) => {
    const isBlock = /language-/.test(className ?? "") || String(children).includes("\n");
    const span = isBlock ? null : String(children);
    const att = span && pictures ? attachmentPath(span) : null;
    if (att) return <LibraryImage path={att} alt="Attached picture" />;
    const lib = span && !att ? libraryPath(span) : null;
    if (lib)
      return <FileChip path={lib} onClick={(newTab) => openLibraryPath(lib, newTab)} />;
    return isBlock ? (
      <code className="block p-3 rounded-lg bg-surface-raised text-[13px] font-mono text-foreground overflow-x-auto" {...p}>
        {children}
      </code>
    ) : (
      <code className="px-1.5 py-0.5 rounded bg-surface-raised text-[13px] font-mono text-foreground" {...p}>
        {children}
      </code>
    );
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
  // A link the agent wrote to a file in the library opens in the side panel,
  // the way the same path does in a tool row — `target="_blank"` would hand a
  // `courses/…` href to the webview, which has nowhere to take it but a blank
  // new tab. Web URLs keep the anchor: `AppLayout` catches those in the
  // capture phase and routes them to an in-app browser tab (⌘-click to the
  // real browser), so the markup here stays a plain link on purpose.
  a: ({ href, children, ...p }: any) => {
    const lib = libraryPath(href);
    if (lib) {
      return (
        <button
          type="button"
          onClick={(e) => openLibraryPath(lib, e.metaKey || e.ctrlKey)}
          className="text-left text-brand hover:underline"
          {...p}
        >
          {children}
        </button>
      );
    }
    // A destination with no scheme is a path on this machine, and an anchor
    // can only resolve it against the app's own origin — which is how a link
    // to a file outside the library reached `http://localhost:1420/Users/…`
    // and 404ed. There is nowhere to take it, so it stays the text it was.
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
  // No blanket italic: a pulled quote can run several lines, and italic set at
  // paragraph length is slow to read whatever the face. The rule and the muted
  // ink already mark it as quoted; `em` inside it still italicises normally.
  blockquote: (p: any) => (
    <blockquote className="border-l-2 border-border pl-4 my-3 text-sm text-muted-foreground" {...p} />
  ),
  code: codeRenderer(true),
  // A ```mermaid fence is a diagram, drawn by `Mermaid` — which falls back to
  // exactly this `<pre>` while the fence is still streaming in, and keeps it
  // for good if the source never parses. Caught here rather than in `code`
  // because an SVG belongs beside a `<pre>`, not inside one: `white-space:
  // pre` on the wrapper turns the gaps between mermaid's own elements into
  // rendered whitespace.
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

/** The plugin sets, hoisted so a re-render hands `ReactMarkdown` the same array
 *  identity it had last time. KaTeX is the expensive part of the pipeline and
 *  most text has no maths at all, so both are gated on [`MATH`]. */
const PLAIN = [remarkGfm];
const WITH_MATH = [remarkGfm, remarkMath];
const KATEX = [rehypeKatex];
const NO_PLUGINS: never[] = [];

/**
 * Markdown with no block elements in the output — every paragraph is a
 * `<span>`, and a heading, list or fence is unwrapped to its text.
 *
 * **This exists because of where it is used.** A line of the enhanced
 * transcript (`ReadingList`) and a chapter's summary (`ChaptersPanel`) are
 * each inside a `<button>` that seeks, and
 * a `<p>` or `<ul>` inside a button is invalid HTML that WebKit resolves by
 * closing the button early — the tail of the text ends up outside the control
 * that seeks. So the one thing these actually carry, inline maths, gets
 * rendered, and the block grammar a one-sentence line was never going to use
 * is flattened rather than risked.
 */
export function InlineMd({ text, className }: { text: string; className?: string }) {
  const math = MATH.test(text);
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
}

/** Unwrapped rather than dropped: the text inside a stray heading is still the
 *  sentence someone wrote, and losing it silently is worse than losing its
 *  weight. */
const BLOCKS = ["h1", "h2", "h3", "h4", "h5", "h6", "hr", "img", "table", "blockquote"];

/** `p` and `li` become inline so nothing block-level reaches the DOM; the rest
 *  of the vocabulary — emphasis, code, links, maths — is already inline and
 *  comes from the shared set. */
const INLINE_COMPONENTS: Components = {
  ...MD_COMPONENTS,
  // No pictures from a fenced attachment path either: `BLOCKS` stops the
  // markdown grammar from reaching `img`, and this stops the `code` handler
  // from drawing one behind its back ([`codeRenderer`]).
  code: codeRenderer(false),
  p: (p: any) => <span {...p} />,
  ul: (p: any) => <span {...p} />,
  ol: (p: any) => <span {...p} />,
  li: (p: any) => <span {...p} />,
  pre: (p: any) => <span {...p} />,
};

/**
 * Full markdown at the size a docked panel can hold — the chat timeline's
 * replies.
 *
 * The components in this file are sized for a document (the file viewer), and
 * `.md-compact` in `index.css` is the one block that pulls that scale down,
 * including KaTeX's own metrics: a display formula in a 220px dock needs a
 * scroller of its own or it widens the panel it is in.
 */
export function CompactMd({ text, className }: { text: string; className?: string }) {
  const math = MATH.test(text);
  return (
    <div className={cn("md-compact min-w-0", className)}>
      <ReactMarkdown
        remarkPlugins={math ? WITH_MATH : PLAIN}
        rehypePlugins={math ? KATEX : NO_PLUGINS}
        components={MD_COMPONENTS}
      >
        {math ? normalizeMath(text) : text}
      </ReactMarkdown>
    </div>
  );
}
