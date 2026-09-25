import { useEffect, useState } from "react";
import { cn } from "@/lib/utils";

/**
 * A number field written only on blur ("90" is typed through "9"). The draft
 * resyncs on `value`, not the row object, so an unrelated write doesn't yank
 * text from under the cursor.
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
