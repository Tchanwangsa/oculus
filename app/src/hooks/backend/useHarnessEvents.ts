import { useRef } from "react";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import type { HarnessEnvelope } from "@/lib/harness";
import { useTauriEvent } from "@/hooks/backend/useEvents";
import { notifyProjectsUpdated } from "@/lib/planning/projects";

const WRITES_PLANNING = /\boculus\s+(project|task|tag)\b/;

/** Feeds the chat store and tells the planning board to re-read when an agent
 *  writes to it. */
export function useHarnessEvents() {
  // An agent's `oculus project`/`task`/`tag` writes from another process,
  // so the board is told to re-read when such a call finishes (ids are
  // remembered from `tool_started`; `tool_finished` has no command). Match
  // the command text, not `kind === "oculus_cli"`: `is_oculus_cli`
  // (`harness/event/classify.rs`) misses `cd … && oculus task add`.
  const revisionCalls = useRef(new Set<string>()).current;
  const planningCalls = useRef(new Set<string>()).current;
  useTauriEvent<HarnessEnvelope>("harness-event", (e) => {
    useHarnessStore.getState().apply(e.payload);
    const ev = e.payload.event;
    if (ev.type === "tool_started") {
      // `title` is the whole command for a Bash-shaped tool.
      if (/\boculus\s+(cheat|quiz|artifact|topic)\b/.test(ev.title)) revisionCalls.add(`${e.payload.threadId}:${ev.id}`);
      if (WRITES_PLANNING.test(ev.title)) {
        planningCalls.add(`${e.payload.threadId}:${ev.id}`);
      }
    } else if (ev.type === "tool_finished") {
      if (revisionCalls.delete(`${e.payload.threadId}:${ev.id}`)) window.dispatchEvent(new CustomEvent("oculus:revision-updated"));
      // Regardless of `ok`: a timed-out command may still have landed.
      if (planningCalls.delete(`${e.payload.threadId}:${ev.id}`)) {
        notifyProjectsUpdated();
      }
    }
  });
}
