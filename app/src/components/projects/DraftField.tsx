import { useEffect, useState } from "react";
import { cn } from "@/lib/utils";

/**
 * Editing a plain number on a project row — a task's estimate, and whatever
 * comes next.
 *
 * Dates used to go through here too, as a `type="datetime-local"`. They now
 * have a control of their own (`./DateTimeField.tsx`), because the native one
 * rendered as five separately-hovering segments and opened the OS calendar;
 * what is left here is the part that was never the problem.
 */

// ── The field ────────────────────────────────────────────────────────────────

/**
 * A field that is only written when you leave it.
 *
 * Committing on every change cannot work here: "90" is typed through "9", so a
 * write per keystroke stores a number nobody asked for and, on a cleared
 * field, a `null` mid-edit. The draft therefore commits on blur, and follows
 * the row only when the row itself changes — which is also what keeps a
 * `PROJECTS_UPDATED_EVENT` from someone else's write (the chat agent's through
 * the CLI, another tab's) from yanking the characters out from under the
 * cursor. The dependency is the *value*, not the row object, so an unrelated
 * project write re-rendering this page does not disturb it either.
 */
export function DraftField({
  value,
  placeholder,
  className,
  onCommit,
}: {
  value: string;
  placeholder?: string;
  className?: string;
  onCommit: (next: string) => void;
}) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);

  return (
    <input
      type="number"
      value={draft}
      placeholder={placeholder}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => {
        if (draft !== value) onCommit(draft);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") e.currentTarget.blur();
        if (e.key === "Escape") {
          setDraft(value);
          e.currentTarget.blur();
        }
      }}
      className={cn(
        "rounded-md border border-transparent bg-transparent px-1.5 py-1 text-xs text-foreground outline-none transition-colors",
        "hover:border-border-subtle hover:bg-surface focus:border-brand/40 focus:bg-card",
        "placeholder:text-muted-foreground/60",
        className,
      )}
    />
  );
}
