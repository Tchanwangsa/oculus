import { CodeText } from "@/components/markdown/MdComponents";
import { diffLines, type DiffRun } from "@/lib/ed/feedback";
import { cn } from "@/lib/utils";

const RUN_CLASS: Record<Exclude<DiffRun["kind"], "missingNewline">, string> = {
  observed: "rounded-[3px] bg-destructive/15 text-destructive",
  expected: "rounded-[3px] bg-success/15 text-success",
  same: "",
};

/** A `check_diff` as Ed draws it: your output's words (`-`) red, the
 *  expected words (`+`) green, the rest plain, and a missing final newline
 *  marked where it belongs. */
export function DiffBlock({ diffs }: { diffs: Array<Array<[string, string]>> }) {
  const lines = diffLines(diffs);
  return (
    <div className="space-y-1">
      <p className="flex gap-3 text-[11px] text-muted-foreground">
        <span className="text-destructive">− Your output</span>
        <span className="text-success">+ Expected</span>
      </p>
      <CodeText className="max-h-80 overflow-auto rounded-md bg-surface px-3 py-2">
        {lines.map((runs, i) => (
          <div key={i} className="min-h-[1.5em]">
            {runs.map((run, j) =>
              run.kind === "missingNewline" ? (
                <span
                  key={j}
                  className="ml-1 rounded-full bg-warning/15 px-1.5 py-px align-middle font-sans text-[10px] text-warning"
                >
                  missing newline
                </span>
              ) : (
                <span key={j} className={cn(RUN_CLASS[run.kind])}>
                  {run.text}
                </span>
              ),
            )}
          </div>
        ))}
      </CodeText>
    </div>
  );
}
