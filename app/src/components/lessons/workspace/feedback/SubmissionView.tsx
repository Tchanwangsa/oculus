import { AnsiText } from "@/components/markdown/MdComponents";
import { submissionSummary, summaryText } from "@/lib/ed/feedback";
import { fmtFullStamp } from "@/lib/format/format";
import type { Submission } from "@/lib/ed/workspace";

import { TestcaseRow } from "./TestcaseRow";

/** One submission as Ed's feedback panel shows it: "N / M passed, P points",
 *  Ed's overall feedback, a failed build's output, then each testcase. */
export function SubmissionView({ submission: sub }: { submission: Submission }) {
  const result = sub.result;
  const stamp = Date.parse(sub.markedAt ?? sub.createdAt);
  const feedback = sub.feedback ?? result?.feedback;
  const status = sub.status.charAt(0).toUpperCase() + sub.status.slice(1);
  return (
    <div className="space-y-3">
      <div>
        <p className="font-display text-[15px] font-semibold tabular-nums">
          {result ? summaryText(submissionSummary(sub)) : "Not marked yet"}
        </p>
        <p className="text-xs text-muted-foreground">
          {status}
          {Number.isFinite(stamp) && ` · ${fmtFullStamp(stamp)}`}
        </p>
      </div>
      {feedback && (
        <p data-selectable className="whitespace-pre-wrap text-[13px]">
          {feedback}
        </p>
      )}
      {result?.build && !result.build.ok && (
        <div className="space-y-1">
          <p className="text-xs font-medium text-destructive">The build failed</p>
          <AnsiText lines={result.build.output.replace(/\n$/, "").split("\n")} className="max-h-64 overflow-auto rounded-md bg-surface px-3 py-2" />
        </div>
      )}
      {result && result.testcases.length > 0 && (
        <ul className="divide-y divide-border-subtle">
          {result.testcases.map((tc, i) => (
            <TestcaseRow key={`${i}-${tc.name}`} testcase={tc} />
          ))}
        </ul>
      )}
    </div>
  );
}
