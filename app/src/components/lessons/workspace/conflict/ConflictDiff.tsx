import { useMemo } from "react";

import { CodeText } from "@/components/markdown/MdComponents";
import { foldUnchanged, lineDiff, type LineDiffKind } from "@/lib/ed/lineDiff";
import { cn } from "@/lib/utils";

const LINE: Record<LineDiffKind, { sign: string; className: string }> = {
  removed: { sign: "−", className: "rounded-[3px] bg-destructive/15 text-destructive" },
  added: { sign: "+", className: "rounded-[3px] bg-success/15 text-success" },
  same: { sign: " ", className: "" },
};

/** Your file against Ed's copy, read-only, line by line in the feedback
 *  diff's colours: lines only yours red, only Ed's green, long unchanged
 *  runs folded. */
export function ConflictDiff({ mine, eds }: { mine: string; eds: string }) {
  const rows = useMemo(() => foldUnchanged(lineDiff(mine, eds)), [mine, eds]);
  const same = rows.every((r) => r.kind === "same" || r.kind === "fold");
  return (
    <div className="flex h-full min-h-0 flex-col">
      <p className="flex shrink-0 gap-3 px-3 pt-2 text-[11px] text-muted-foreground">
        <span className="text-destructive">− Yours</span>
        <span className="text-success">+ Ed's</span>
      </p>
      {same ? (
        <p className="px-3 py-2 text-xs text-muted-foreground">The two versions are the same.</p>
      ) : (
        <CodeText className="min-h-0 flex-1 overflow-auto px-3 py-2">
          {rows.map((row, i) =>
            row.kind === "fold" ? (
              <div
                key={i}
                className="my-1 select-none rounded-[3px] bg-surface px-2 font-sans text-[11px] tabular-nums text-muted-foreground"
              >
                {row.count} unchanged lines
              </div>
            ) : (
              <div key={i} className={cn("min-h-[1.5em]", LINE[row.kind].className)}>
                <span className="inline-block w-4 select-none opacity-70">{LINE[row.kind].sign}</span>
                {row.text}
              </div>
            ),
          )}
        </CodeText>
      )}
    </div>
  );
}
