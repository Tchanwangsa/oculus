import { useEffect, useRef, useState } from "react";

import { flushForEd } from "@/lib/ed/flushForEd";
import { markPrompts, type MarkPrompt } from "@/lib/ed/markPrompts";
import { challengeStanding, markWorkspace, type Submission } from "@/lib/ed/workspace";
import type { WorkspaceSaver } from "@/lib/ed/workspaceSaver";

export type MarkStep =
  | { step: "idle" }
  | { step: "checking" }
  | { step: "confirm"; prompts: MarkPrompt[]; dueAt: string | null }
  | { step: "marking" };

export interface MarkError {
  title: string;
  message: string;
}

/**
 * Mark, only ever from its button: write pending saves, read the live standing
 * (`ed_challenge_standing`), ask what `markPrompts` says to ask in one dialog,
 * then submit (`ed_workspace_mark`). Any failure stops the flow as an error
 * for an inline Alert; nothing is retried, and nothing marks without a click.
 */
export function useMarkFlow({
  challengeId,
  saver,
  confirmSubmit,
  dueAt,
  onMarked,
}: {
  challengeId: number;
  saver: WorkspaceSaver | null;
  confirmSubmit: boolean;
  /** The due date the workspace opened with; the live one wins. */
  dueAt: string | null;
  onMarked: (submission: Submission) => void;
}) {
  const [state, setState] = useState<MarkStep>({ step: "idle" });
  const [error, setError] = useState<MarkError | null>(null);
  const mounted = useRef(true);
  /** Set from the click until the flow ends, so a double click (state not
   *  yet re-rendered) can't submit twice. */
  const busy = useRef(false);
  /** The submission itself, once; a second Submit click is ignored. */
  const submitting = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  async function submit() {
    if (submitting.current) return;
    submitting.current = true;
    busy.current = true;
    setState({ step: "marking" });
    try {
      const submission = await markWorkspace(challengeId);
      if (mounted.current) onMarked(submission);
    } catch (e) {
      if (mounted.current) setError({ title: "Mark failed", message: String(e) });
    } finally {
      submitting.current = false;
      busy.current = false;
      if (mounted.current) setState({ step: "idle" });
    }
  }

  function stop(err: MarkError) {
    busy.current = false;
    if (!mounted.current) return;
    setError(err);
    setState({ step: "idle" });
  }

  async function start() {
    if (busy.current || !saver) return;
    busy.current = true;
    setError(null);
    setState({ step: "checking" });
    const blocked = await flushForEd(saver);
    if (blocked) return stop({ title: "Not marked", message: blocked });
    let prompts: MarkPrompt[];
    let due: string | null;
    try {
      const standing = await challengeStanding(challengeId);
      due = standing.dueAt ?? dueAt;
      prompts = markPrompts({
        now: Date.now(),
        dueAt: due,
        confirmSubmit,
        attemptsRemaining: standing.attemptsRemaining,
      });
    } catch (e) {
      return stop({ title: "Couldn't check the challenge on Ed", message: String(e) });
    }
    if (!mounted.current) {
      busy.current = false;
      return;
    }
    if (prompts.length > 0) setState({ step: "confirm", prompts, dueAt: due });
    else await submit();
  }

  return {
    state,
    error,
    dismissError: () => setError(null),
    start,
    confirm() {
      if (state.step === "confirm") void submit();
    },
    cancel() {
      if (state.step !== "confirm" || submitting.current) return;
      busy.current = false;
      setState({ step: "idle" });
    },
  };
}
