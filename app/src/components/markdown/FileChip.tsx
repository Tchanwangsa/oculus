import type { KeyboardEvent, MouseEvent } from "react";
import { categoryIconFor } from "@/lib/fileTypes";
import { fileTitle, pathFile } from "@/lib/openFile";
import { cn } from "@/lib/utils";

/**
 * A file named inside a line of text, drawn as itself: its own glyph and its
 * display name instead of the library path the message actually carries.
 *
 * **One definition, three places**, because a mention has to look the same
 * everywhere it appears or it reads as three different things: the composer's
 * box while it is being typed (`app/src/components/harness/MentionInput.tsx`),
 * the question bubble after it is sent (`app/src/components/harness/Timeline.tsx`),
 * and a path the agent quotes back in its own prose (`MdComponents.tsx`). The
 * *message* is the backticked path in all three — `oculus read` needs the
 * path, not a title — so this is a rendering of text that never stops being
 * text.
 *
 * It takes a path and nothing else. The label and the glyph are derived from
 * the path (`pathFile`), so a reader can draw a hundred chips without a
 * single query, which is the same rule the timeline's tool rows already
 * follow: matched on shape, resolved on click.
 *
 * Two shapes are deliberate. `display: inline` rather than a flex pill,
 * because this sits in a line of prose: an inline box contributes only its
 * line-height, so the padding that makes it read as a chip cannot push the
 * lines of the paragraph apart — and it is the app's one soft rectangle
 * rather than a `rounded-full` pill, which a run of them in a sentence reads
 * as a string of lozenges. And a clickable chip is a **span**, not a button:
 * `InlineMd` renders inside a `<button>` that seeks, where a nested button is
 * invalid markup WebKit fixes by closing the outer control early and
 * stranding the rest of the line outside it.
 */
export function FileChip({
  path,
  onClick,
  className,
}: {
  /** The library path, `courses/<CODE>/…` — what the message says. */
  path: string;
  /** Opening the file, where a chip is a control. Omitted in the composer,
   *  where a click belongs to the caret. `newTab` is the ⌘-click: a chip holds
   *  a library path rather than a route, so this is the one place the modifier
   *  has to be carried by hand instead of by `data-tab-href`
   *  (`app/src/lib/newTabClicks.ts`). */
  onClick?: (newTab: boolean) => void;
  className?: string;
}) {
  const file = pathFile(path);
  const Icon = categoryIconFor(file);
  return (
    <span
      // Atomic wherever it is: inside the composer's editable box this is what
      // makes the chip one object to the caret and `data-path` is how the box
      // is read back into a message; everywhere else both are inert.
      contentEditable={false}
      data-path={path}
      // The chip is the path made short, so the path itself is what hovering
      // it says — the one place the full text is still reachable by eye.
      title={path}
      {...(onClick
        ? {
            role: "button",
            tabIndex: 0,
            // A chip can sit inside a control of its own (a transcript line
            // that seeks), and opening the file is not also asking for that.
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
