import { useMemo, useState } from "react";
import { CaretDown } from "@phosphor-icons/react";
import { CodeText } from "@/components/markdown/MdComponents";
import { diffLines, type DiffLine } from "@/lib/harness";
import { cn } from "@/lib/utils";
import { useOverflows } from "../useOverflows";

/** Taller than this folds behind Show more: the timeline is the page's only scroller. */
const DIFF_MAX_H = 240;

const TONE: Record<DiffLine["tone"], string> = {
  add: "bg-success/10 text-success",
  remove: "bg-destructive/10 text-destructive",
  hunk: "text-muted-foreground",
  context: "text-foreground/80",
};

/** A unified diff, its added and removed lines tinted, sized to content up
 *  to `DIFF_MAX_H` with Show more past it. */
export function DiffBlock({ diff }: { diff: string }) {
  const lines = useMemo(() => diffLines(diff), [diff]);
  const [open, setOpen] = useState(false);
  const [body, long] = useOverflows(diff, true, DIFF_MAX_H);
  const clipped = long && !open;
  return (
    <div className="mt-1 rounded-md bg-surface">
      <div
        ref={body}
        className={cn(
          "py-1.5",
          clipped && "overflow-hidden [mask-image:linear-gradient(to_bottom,#000_calc(100%-2.25rem),transparent)]",
        )}
        style={clipped ? { maxHeight: DIFF_MAX_H } : undefined}
      >
        <CodeText className="text-[11.5px]">
          {lines.map((line, i) => (
            <span key={i} className={cn("block px-2.5", TONE[line.tone])}>
              {line.text || " "}
            </span>
          ))}
        </CodeText>
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
