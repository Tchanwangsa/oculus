import { useEffect, useRef, useState } from "react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { LoadingFill } from "@/components/ui/layout/PageParts";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { workspaceSubmissions, type Submission } from "@/lib/ed/workspace";

import { SubmissionHistory } from "./SubmissionHistory";
import { SubmissionView } from "./SubmissionView";

/**
 * Ed's Feedback tab: the submissions this app made (`ed_workspace_submissions`,
 * newest first), loaded the first time the tab opens; one shown in full and
 * the rest listed to pick from. A new mark (`marked`) joins the list and is
 * shown at once, without reloading the others.
 */
export function FeedbackPanel({
  challengeId,
  active,
  marked,
}: {
  challengeId: number;
  /** The tab is showing. */
  active: boolean;
  /** The submission the last Mark returned. */
  marked: Submission | null;
}) {
  const [list, setList] = useState<Submission[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<number | null>(null);
  const scroller = useRef<HTMLDivElement>(null);
  useScrollFade(scroller, "y");
  const requested = useRef(false);
  const latestMark = useRef(marked);
  latestMark.current = marked;

  // Once; a failed load tries again the next time the tab opens.
  useEffect(() => {
    if (!active || requested.current) return;
    requested.current = true;
    setError(null);
    workspaceSubmissions(challengeId)
      .then((subs) => {
        const mark = latestMark.current;
        setList(mark && !subs.some((s) => s.id === mark.id) ? [mark, ...subs] : subs);
      })
      .catch((e) => {
        requested.current = false;
        setError(String(e));
      });
  }, [active, challengeId]);

  useEffect(() => {
    if (!marked) return;
    setList((prev) => (prev ? [marked, ...prev.filter((s) => s.id !== marked.id)] : prev));
    setSelected(marked.id);
  }, [marked]);

  const subs = list ?? [];
  const shown = subs.find((s) => s.id === selected) ?? subs[0] ?? null;

  return (
    <div ref={scroller} className="h-full overflow-y-auto">
      <div className="space-y-5 px-4 py-3">
        {error && (
          <Alert variant="destructive">
            <AlertTitle className="text-xs">Couldn't load your submissions</AlertTitle>
            <AlertDescription className="text-xs">{error}</AlertDescription>
          </Alert>
        )}
        {list === null && !error && (
          <div className="h-24">
            <LoadingFill />
          </div>
        )}
        {list !== null && subs.length === 0 && (
          <p className="text-sm text-muted-foreground">
            No submissions from Oculus yet. Mark submits your saved files and the feedback shows here.
          </p>
        )}
        {shown && <SubmissionView submission={shown} />}
        {subs.length > 1 && shown && (
          <SubmissionHistory submissions={subs} selected={shown.id} onSelect={setSelected} />
        )}
      </div>
    </div>
  );
}
