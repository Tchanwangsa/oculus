import type { KeyboardEvent, MouseEvent } from "react";
import { categoryIconFor } from "@/lib/files/fileTypes";
import { fileTitle, pathFile } from "@/lib/files/openFile";
import { citationText, parsedSourceOf, type Citation } from "@/lib/citations";
import { useCitedPage } from "@/hooks/agents/useCitation";
import { cn } from "@/lib/utils";

/** The chip's location suffix: a page, else a line range. */
function locationLabel(cite: Citation | undefined, page: number | null): string {
  const p = page ?? cite?.page;
  if (p) return ` · p. ${p}`;
  if (!cite?.line) return "";
  const { from, to } = cite.line;
  return ` · L${from}${to !== from ? `–${to}` : ""}`;
}

/**
 * A library path drawn as a file mention (glyph + display name), shared by the
 * composer, the question bubble and agent prose. Label and glyph derive from
 * the path alone, so no query. `display: inline` so its padding cannot push
 * lines apart; a span, not a button, because it can sit inside `InlineMd`'s
 * seeking `<button>`.
 */
export function FileChip({
  path,
  cite,
  onClick,
  className,
}: {
  /** The library path, `courses/<CODE>/…` — what the message says. */
  path: string;
  /** Where in the file an agent's citation points; adds ` · p. 12` or
   *  ` · L97–120` to the label, and copies as the whole citation. */
  cite?: Citation;
  /** Omitted in the composer, where a click belongs to the caret. `newTab` is
   *  ⌘-click, carried by hand since a path is not a `data-tab-href` route. */
  onClick?: (newTab: boolean) => void;
  className?: string;
}) {
  // A parse artifact is named for the PDF or Office file the student knows,
  // and its line becomes that file's page once `.pages.json` is read.
  const file = pathFile(parsedSourceOf(path) ?? path);
  const Icon = categoryIconFor(file);
  const page = useCitedPage(cite);
  const full = cite ? citationText(cite) : path;
  return (
    <span
      // In the composer: atomic to the caret, and `data-path` is read back
      // into the message. Elsewhere it is what a copy writes.
      contentEditable={false}
      data-path={full}
      title={full}
      {...(onClick
        ? {
            role: "button",
            tabIndex: 0,
            // Don't also trigger an enclosing control (a seeking line).
            onClick: (e: MouseEvent) => {
              e.stopPropagation();
              onClick(e.metaKey || e.ctrlKey);
            },
            onKeyDown: (e: KeyboardEvent) => {
              if (e.key !== "Enter" && e.key !== " ") return;
              e.preventDefault();
              e.stopPropagation();
              onClick(e.metaKey || e.ctrlKey);
            },
          }
        : {})}
      className={cn(
        "mx-px inline rounded bg-accent px-1 whitespace-nowrap text-accent-foreground",
        onClick && "cursor-pointer hover:bg-surface-overlay",
        className,
      )}
    >
      <Icon size={12} className="mr-1 inline align-[-2px] text-muted-foreground" />
      {fileTitle(file)}
      {locationLabel(cite, page)}
    </span>
  );
}
