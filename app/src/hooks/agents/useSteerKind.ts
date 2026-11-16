import { useShallow } from "zustand/react/shallow";
import { liveRunId, steerKind, waitingQuestions, type Provider, type SteerKind } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { useCapabilities } from "./useCapabilities";

/**
 * How `threadId`'s running turn can take a message now (`steerKind`): read
 * from the run on screen and its own bridge, so a pick in the composer for
 * the next send never changes it. Null on an idle chat, a turn the CLI
 * started itself, a run being stopped, or while a question waits.
 */
export function useSteerKind(threadId: number | null): SteerKind {
  const s = useHarnessStore(
    useShallow((st) => {
      if (threadId == null) return { running: false, run: null, stopping: false, provider: null, question: false };
      const runs = st.runs[threadId];
      const id = liveRunId(runs);
      const run = id == null ? undefined : runs?.[id];
      const provider: Provider | null = run?.provider ?? st.threads.find((t) => t.id === threadId)?.provider ?? null;
      return {
        running: st.live[threadId]?.running ?? false,
        run: run?.id ?? null,
        stopping: !!run?.stopping,
        provider,
        question: waitingQuestions(st.requests[threadId]).length > 0,
      };
    }),
  );
  // Hooks run unconditionally; with no provider the caps are never read.
  const caps = useCapabilities(s.provider ?? "claude");
  if (s.provider == null) return null;
  return steerKind({
    running: s.running,
    liveRun: s.run == null ? null : { stopping: s.stopping },
    caps,
    questionWaits: s.question,
  });
}
