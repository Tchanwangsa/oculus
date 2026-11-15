import { useMemo } from "react";
import { useSwitchLock } from "@/hooks/agents/useCapabilities";
import { pendingSwitch, threadSelection, type HarnessThread, type Provider, type Selection } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { usePickStore } from "@/stores/chat/pickStore";

/**
 * What a chat surface's composer names, and what its picker needs to say
 * about it (`docs/harness.md`, "In the composer and the timeline"): the open
 * thread's selection with any held pick over it, else the session's for a
 * new thread; Rust's last refusal; the agent a pending switch leaves; and
 * the agents this chat cannot move to.
 */
export function useComposerSelection(thread: HarnessThread | null): {
  selection: Selection;
  refused: string | null;
  switchFrom: Provider | null;
  locked: (provider: Provider) => boolean;
} {
  const id = thread?.id ?? null;
  const provider = useHarnessStore((s) => s.provider);
  const model = useHarnessStore((s) => s.model);
  const reasoning = useHarnessStore((s) => s.reasoning);
  const held = usePickStore((s) => (id == null ? undefined : s.held[id]));
  const refused = usePickStore((s) => (id == null ? null : (s.refused[id] ?? null)));
  const open = thread != null;
  const t = { provider: thread?.provider ?? provider, model: thread?.model ?? null, effort: thread?.effort ?? null };
  const selection = useMemo(
    () => (open ? threadSelection(t, held, reasoning) : { provider, model, effort: reasoning }),
    // `t` is rebuilt every render; its fields are what matter.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [open, t.provider, t.model, t.effort, held, provider, model, reasoning],
  );
  const switchFrom = useHarnessStore((s) => (id == null ? null : pendingSwitch(selection, s.runs[id])));
  const locked = useSwitchLock(thread?.provider ?? null);
  return { selection, refused, switchFrom, locked };
}
