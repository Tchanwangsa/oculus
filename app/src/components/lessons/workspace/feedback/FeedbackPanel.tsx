import { useEffect, useRef, useState } from "react";
import { ArrowCounterClockwise } from "@phosphor-icons/react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { LoadingFill } from "@/components/ui/layout/PageParts";
import { PillTabs } from "@/components/ui/table/PillTabs";
import { useTauriEvent } from "@/hooks/backend/useEvents";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { fmtFullStamp } from "@/lib/format/format";
import {
  WORKSPACE_EVENT,
  restoreSubmission,
  workspaceSubmissions,
  type Submission,
  type WorkspaceEvent,
} from "@/lib/ed/workspace";
import { heldSaver } from "@/lib/ed/workspaceHandles";
import { completesSlide, recordSlideStatus } from "@/lib/ed/slideStatus";
import { edUnavailableReason } from "@/lib/ed/workspaceStatus";

import { useReplaceFlow } from "../reset/useReplaceFlow";
import { SubmissionCode } from "./SubmissionCode";
import { SubmissionHistory } from "./SubmissionHistory";
import { SubmissionView } from "./SubmissionView";

type View = "feedback" | "code";

const VIEWS: ReadonlyArray<{ value: View; label: string }> = [
  { value: "feedback", label: "Feedback" },
  { value: "code", label: "Code" },
];

/**
 * Ed's Feedback tab: the student's submissions as Ed lists them
 * (`ed_workspace_submissions`, newest first), loaded the first time the tab
 * opens; one shown in full — its feedback, or its code read-only — and all
 * listed to pick from. Restore replaces the workspace with the shown one's
 * code, after asking. A new mark (`marked`) joins the list and is shown at
 * once, without reloading the others. A passed submission ticks the slide.
 */
export function FeedbackPanel({
  challengeId,
  slideId,
  active,
  marked,
}: {
  challengeId: number;
  /** The challenge's Ed slide, ticked in the lesson sidebar once a submission passes. */
  slideId: number | null;
  /** The tab is showing. */
  active: boolean;
  /** The submission the last Mark returned. */
  marked: Submission | null;
}) {
  const [list, setList] = useState<Submission[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<number | null>(null);
  const [view, setView] = useState<View>("feedback");
  /** Learnt from status events; unknown counts as online and Rust refuses. */
  const [offline, setOffline] = useState<string | null>(null);
  useTauriEvent<WorkspaceEvent>(WORKSPACE_EVENT, ({ payload }) => {
    if (payload.challengeId === challengeId && payload.kind === "status") {
      setOffline(edUnavailableReason(payload.status));
    }
  });
  const restore = useReplaceFlow<Submission>(heldSaver(challengeId), (sub) => restoreSubmission(challengeId, sub.id));
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

  const completed = list?.some((s) => completesSlide(s.status)) ?? false;
  useEffect(() => {
    if (completed && slideId != null) recordSlideStatus(slideId, "completed");
  }, [completed, slideId]);

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
        {restore.error && (
          <Alert variant="destructive">
            <AlertTitle className="text-xs">Couldn't restore the submission</AlertTitle>
            <AlertDescription className="text-xs">{restore.error}</AlertDescription>
          </Alert>
        )}
        {list !== null && subs.length === 0 && (
          <p className="text-sm text-muted-foreground">
            No submissions yet. Mark submits your saved files and the feedback shows here.
          </p>
        )}
        {shown && (
          <div className="flex items-center gap-2">
            <PillTabs tabs={VIEWS} value={view} onChange={setView} />
            <div className="flex-1" />
            <span title={offline ?? undefined} className="shrink-0">
              <Button size="xs" variant="secondary" disabled={offline != null || restore.busy} onClick={() => restore.ask(shown)}>
                <ArrowCounterClockwise aria-hidden />
                Restore
              </Button>
            </span>
          </div>
        )}
        {shown && view === "feedback" && <SubmissionView submission={shown} />}
        {shown && view === "code" && <SubmissionCode challengeId={challengeId} submissionId={shown.id} />}
        {subs.length > 1 && shown && (
          <SubmissionHistory submissions={subs} selected={shown.id} onSelect={setSelected} />
        )}
        <ConfirmDialog
          open={restore.asking != null}
          onCancel={restore.cancel}
          onConfirm={restore.confirm}
          title="Restore this submission?"
          description={`Your current files will be replaced by the code you submitted on ${restoreDate(restore.asking)}.`}
          confirmLabel="Restore"
          busyLabel="Restoring…"
          busy={restore.busy}
        />
      </div>
    </div>
  );
}

function restoreDate(sub: Submission | null): string {
  const at = Date.parse(sub?.createdAt ?? "");
  return Number.isFinite(at) ? fmtFullStamp(at) : "that date";
}
