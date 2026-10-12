import { CheckCircle, MinusCircle, XCircle } from "@phosphor-icons/react";

import { AnsiText } from "@/components/markdown/MdComponents";
import { testcaseShown } from "@/lib/ed/feedback";
import type { SubmissionTestcase } from "@/lib/ed/workspace";

import { DiffBlock } from "./DiffBlock";

/** One testcase: ✓ or ✗ and its name, then (unless hidden or private, which
 *  show the name only) Ed's feedback, its output and its diffs. */
export function TestcaseRow({ testcase: tc }: { testcase: SubmissionTestcase }) {
  const shown = testcaseShown(tc);
  return (
    <li className="space-y-2 py-3">
      <div className="flex items-start gap-2">
        {tc.skip ? (
          <MinusCircle size={16} className="mt-px shrink-0 text-muted-foreground" aria-label="Skipped" />
        ) : tc.passed ? (
          <CheckCircle size={16} weight="fill" className="mt-px shrink-0 text-success" aria-label="Passed" />
        ) : (
          <XCircle size={16} weight="fill" className="mt-px shrink-0 text-destructive" aria-label="Failed" />
        )}
        <span className="min-w-0 flex-1 text-[13px] font-medium">{tc.name}</span>
        {tc.maxScore != null && (
          <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
            {tc.score ?? 0} / {tc.maxScore}
          </span>
        )}
      </div>
      {shown && tc.feedback && (
        <p data-selectable className="whitespace-pre-wrap pl-6 text-xs text-muted-foreground">
          {tc.feedback}
        </p>
      )}
      {shown && tc.output && (
        <div className="pl-6">
          <AnsiText lines={tc.output.replace(/\n$/, "").split("\n")} className="max-h-64 overflow-auto rounded-md bg-surface px-3 py-2" />
        </div>
      )}
      {shown && tc.diffs.length > 0 && (
        <div className="pl-6">
          <DiffBlock diffs={tc.diffs} />
        </div>
      )}
    </li>
  );
}
