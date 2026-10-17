import { useCallback, type KeyboardEvent, type MouseEvent, type ReactNode } from "react";
import { fileReferenceAppearance } from "@/lib/files/referenceAppearance";
import { fileTitle, pathFile } from "@/lib/files/openFile";
import { citationText, parsedSourceOf, type Citation } from "@/lib/citations";
import { useCitedPage } from "@/hooks/agents/useCitation";
import { cn } from "@/lib/utils";
import { ReferencePreview, referenceClass, referenceRestClass } from "./ReferencePreview";
import { fileReferenceDetails } from "./referenceDetails";

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
 * the path; hover metadata loads on demand. `display: inline` so padding cannot push
 * lines apart; a span, not a button, because it can sit inside `InlineMd`'s
 * seeking `<button>`.
 */
export function FileChip({
  path,
  cite,
  onClick,
  className,
  children,
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
  children?: ReactNode;
}) {
  // A parse artifact is named for the PDF or Office file the student knows,
  // and its line becomes that file's page once `.pages.json` is read.
  const file = pathFile(parsedSourceOf(path) ?? path);
  const { Icon, tone, kind } = fileReferenceAppearance(file, path);
  const page = useCitedPage(cite);
  const full = cite ? citationText(cite) : path;
  const label = children ?? fileTitle(file);
  const load = useCallback(() => fileReferenceDetails(path), [path]);
  const icon = <Icon size="1.15em" aria-hidden className={cn("mr-[0.35em] inline align-[-0.18em]", tone)} />;
  return (
    <ReferencePreview icon={icon} title={fileTitle(file)} kind={kind} detail={full} load={load}>
      <span
        // In the composer: atomic to the caret, and `data-path` is read back
        // into the message. Elsewhere it is what a copy writes.
        contentEditable={onClick ? undefined : false}
        data-path={full}
        {...(onClick
          ? {
              role: "link",
              tabIndex: 0,
              // Don't also trigger an enclosing control (a seeking line).
              onClick: (e: MouseEvent) => {
                e.stopPropagation();
                if (window.getSelection()?.isCollapsed === false) return;
                onClick(e.metaKey || e.ctrlKey);
              },
              onAuxClick: (e: MouseEvent) => {
                if (e.button !== 1) return;
                e.preventDefault();
                e.stopPropagation();
                onClick(true);
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
          "mx-px inline box-decoration-clone",
          typeof label === "string" && label.length <= 40 && "whitespace-nowrap",
          onClick ? cn(referenceClass, "cursor-pointer") : referenceRestClass,
          className,
        )}
      >
        {icon}
        {label}
        {!children && locationLabel(cite, page)}
      </span>
    </ReferencePreview>
  );
}
