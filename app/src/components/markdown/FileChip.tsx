import type { KeyboardEvent, MouseEvent } from "react";
import { categoryIconFor } from "@/lib/fileTypes";
import { fileTitle, pathFile } from "@/lib/openFile";
import { cn } from "@/lib/utils";

/**
 * A library path drawn as a file mention (glyph + display name), shared by the
 * composer, the question bubble and agent prose. Label and glyph derive from
 * the path alone, so no query. `display: inline` so its padding cannot push
 * lines apart; a span, not a button, because it can sit inside `InlineMd`'s
 * seeking `<button>`.
 */
export function FileChip({
  path,
  onClick,
  className,
}: {
  /** The library path, `courses/<CODE>/…` — what the message says. */
  path: string;
  /** Omitted in the composer, where a click belongs to the caret. `newTab` is
   *  ⌘-click, carried by hand since a path is not a `data-tab-href` route. */
  onClick?: (newTab: boolean) => void;
  className?: string;
}) {
  const file = pathFile(path);
  const Icon = categoryIconFor(file);
  return (
    <span
      // In the composer: atomic to the caret, and `data-path` is read back
      // into the message. Inert elsewhere.
      contentEditable={false}
      data-path={path}
      title={path}
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
    </span>
  );
}
