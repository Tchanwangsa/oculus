import type { Components } from "react-markdown";
import { ArrowSquareOut } from "@phosphor-icons/react";
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
          onClick={() => openLibraryPath(lib)}
          className="text-left text-brand hover:underline"
          {...p}
        >
          {children}
        </button>
      );
    }
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
  code: ({ className, children, ...p }: any) => {
    const isBlock = /language-/.test(className ?? "") || String(children).includes("\n");
    return isBlock ? (
      <code className="block p-3 rounded-lg bg-surface-raised text-[13px] font-mono text-foreground overflow-x-auto" {...p}>
        {children}
      </code>
    ) : (
      <code className="px-1.5 py-0.5 rounded bg-surface-raised text-[13px] font-mono text-foreground" {...p}>
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
    <th className="border border-border px-3 py-1.5 bg-surface-raised text-left font-semibold text-xs" {...p} />
  ),
  td: (p: any) => (
    <td className="border border-border px-3 py-1.5 text-foreground/90" {...p} />
  ),
};
