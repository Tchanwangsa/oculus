import { useEffect, useRef, useState } from "react";
import { diskName, type DriveItem } from "@/lib/drive/items";
import { nameProblem, stemLength } from "@/lib/drive/paths";
import { cn } from "@/lib/utils";
import { useDriveView } from "../DriveContext";

/**
 * An entry's name, or while it is being renamed a field over it: the stem
 * selected, Enter or leaving the field to keep the new name, Escape to keep
 * the old one. A name Rust would refuse for its shape is said under the
 * field and never sent.
 */
export function EntryName({ item, className }: { item: DriveItem; className?: string }) {
  const view = useDriveView();
  if (view.renaming !== item.key) {
    return <span className={cn("min-w-0 flex-1 truncate", className)} title={item.name}>{item.name}</span>;
  }
  return <RenameField item={item} className={className} />;
}

function RenameField({ item, className }: { item: DriveItem; className?: string }) {
  const view = useDriveView();
  const original = diskName(item);
  const [value, setValue] = useState(original);
  const input = useRef<HTMLInputElement>(null);
  const done = useRef(false);
  const problem = value.trim() === original ? null : nameProblem(value);

  useEffect(() => {
    const el = input.current;
    if (!el) return;
    el.focus();
    el.setSelectionRange(0, stemLength(original, item.type === "folder"));
  }, [original, item.type]);

  const finish = (keep: boolean) => {
    if (done.current) return;
    if (keep && problem) return;
    done.current = true;
    const name = value.trim();
    view.finishRename(item, keep && name && name !== original ? name : null);
  };

  return (
    <span className={cn("relative min-w-0 flex-1", className)}>
      <input
        ref={input}
        value={value}
        aria-label={`Rename ${original}`}
        aria-invalid={problem != null}
        spellCheck={false}
        autoComplete="off"
        onChange={(e) => setValue(e.target.value)}
        onClick={(e) => e.stopPropagation()}
        onDoubleClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Enter") finish(true);
          else if (e.key === "Escape") finish(false);
        }}
        onBlur={() => finish(!problem)}
        className={cn(
          "h-6 w-full rounded-md border bg-card px-1.5 text-[12.5px] text-foreground outline-none [-webkit-user-select:text] [user-select:text]",
          problem ? "border-destructive/60" : "border-ring ring-[3px] ring-ring/25",
        )}
      />
      {problem && (
        <span className="absolute top-full left-0 z-10 mt-1 rounded-md bg-popover px-2 py-1 text-[11px] text-destructive shadow-md">
          {problem}
        </span>
      )}
    </span>
  );
}
