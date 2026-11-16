import { useState } from "react";
import { CaretDown } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { useOverflows } from "../useOverflows";

/** Taller than this folds behind Show more: the timeline is the page's only scroller. */
const FOLD_MAX_H = 240;

/** A code surface sized to its content up to `FOLD_MAX_H`, with Show more
 *  past it. `text` is what it shows, so a change re-measures it. */
export function FoldBlock({ text, children }: { text: string; children: React.ReactNode }) {
  const [open, setOpen] = useState(false);
  const [body, long] = useOverflows(text, true, FOLD_MAX_H);
  const clipped = long && !open;
  return (
    <div className="mt-1 rounded-md bg-surface">
      <div
        ref={body}
        className={cn(
          "py-1.5",
          clipped && "overflow-hidden [mask-image:linear-gradient(to_bottom,#000_calc(100%-2.25rem),transparent)]",
        )}
        style={clipped ? { maxHeight: FOLD_MAX_H } : undefined}
      >
        {children}
      </div>
      {long && (
        <button
          type="button"
          data-copy-skip
          aria-expanded={open}
          onClick={() => setOpen((o) => !o)}
          className="flex cursor-pointer items-center gap-1 px-2.5 pb-1.5 text-[11px] text-muted-foreground transition-colors hover:text-foreground"
        >
          {open ? "Show less" : "Show more"}
          <CaretDown size={11} className={cn("transition-transform will-change-transform", open && "rotate-180")} />
        </button>
      )}
    </div>
  );
}
