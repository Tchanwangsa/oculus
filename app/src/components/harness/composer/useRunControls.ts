import { useEffect, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { liveRunId, queueHeld, runControls } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { useRunCommand } from "./useRunCommand";

/** The open run, reduced to what Stop reads. */
function liveRunOf(runs: Parameters<typeof liveRunId>[0]) {
  const id = liveRunId(runs);
  const run = id == null ? undefined : runs?.[id];
  return run ? { id: run.id, stopping: !!run.stopping } : null;
}

/**
 * The composer's Stop and main button for `threadId`, read from the store:
 * which run Stop names, whether it is already stopping, whether the queue is
 * held. Stop and Resume are sent from here so a failure is said beside them
 * (`error`) and neither button stays pending.
 */
export function useRunControls({
  threadId,
  running,
  hasDraft,
  ready,
  writing,
  onStop,
  onResume,
}: {
  threadId: number | null;
  running: boolean;
  hasDraft: boolean;
  ready: boolean;
  writing: boolean;
  onStop: (runId: number | null) => Promise<void> | void;
  onResume?: () => Promise<void>;
}) {
  const liveRun = useHarnessStore(useShallow((s) => (threadId == null ? null : liveRunOf(s.runs[threadId]))));
  const held = useHarnessStore((s) => threadId != null && queueHeld(s.queued[threadId]));
  const stop = useRunCommand();
  const resume = useRunCommand();
  // A stop that went through, by the run it named: a turn the CLI started
  // has no run to mark, so this is what says it is stopping.
  const [sentFor, setSentFor] = useState<{ thread: number | null; run: number | null } | null>(null);
  useEffect(() => {
    if (!running) setSentFor(null);
  }, [running]);
  const { clear: clearStop } = stop;
  const { clear: clearResume } = resume;
  useEffect(() => {
    clearStop();
    clearResume();
  }, [threadId, clearStop, clearResume]);

  const named = liveRun?.id ?? null;
  const stopSent = stop.pending || (sentFor != null && sentFor.thread === threadId && sentFor.run === named);
  const controls = runControls({
    running,
    liveRun,
    stopSent,
    held: held && !!onResume,
    hasDraft,
    ready,
    writing,
  });
  // Pending only while out: the button reads Resume thread again on failure.
  if (controls.send?.kind === "resume" && resume.pending) controls.send = { ...controls.send, disabled: true };

  return {
    ...controls,
    error: stop.error ? `Couldn't stop: ${stop.error}` : resume.error ? `Couldn't resume: ${resume.error}` : null,
    stopNow: () => {
      const runId = named;
      void stop.run(async () => onStop(runId)).then((ok) => {
        if (ok) setSentFor({ thread: threadId, run: runId });
      });
    },
    resumeNow: () => {
      if (onResume) void resume.run(onResume);
    },
  };
}
