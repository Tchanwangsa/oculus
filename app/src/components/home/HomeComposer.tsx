import { useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { Composer } from "@/components/harness/Composer";
import { harnessSend, type Provider } from "@/lib/harness";
import { useHarnessStore } from "@/stores/harnessStore";

/**
 * The box you came to Home for: the Chat page's composer, with the thread id
 * pinned at `null`.
 *
 * Home never continues a conversation — the one you were in is a row in
 * Continue below, and picking up where you left off is a click on that. So
 * every send here opens a **new** thread and then leaves for `/chat`, which is
 * why nothing on this page is `providerLocked` or `subjectLocked`: those two
 * exist to stop an *open* thread changing the agent or the scope it was bound
 * with, and there is no open thread here. `running`, `usage` and the rest are
 * the empty-thread values for the same reason.
 *
 * No suggestion chips and no hero line above it. ChatPage's empty state has
 * both and earns them — it is a page with nothing else on it — where here the
 * date heading is already the page's title and the four sections below are
 * already four things to do.
 *
 * The subject list the scope picker offers is loaded by `HomePage`, not here:
 * Continue's thread rows resolve their subject codes off the same slice, and a
 * load that lives in one of two readers is a load the other one silently
 * depends on being mounted first.
 *
 * `useNavigate`, not `navigateActive`: every tab builds its own memory router
 * (`app/src/components/tabs/TabPane.tsx`), so a page inside one navigates with
 * the plain hook. `navigateActive` is the shell's, for the sidebar and the
 * palette that live outside all of them.
 */
export function HomeComposer() {
  const store = useHarnessStore;
  const navigate = useNavigate();
  const provider = useHarnessStore((s) => s.provider);
  const model = useHarnessStore((s) => s.model);
  const reasoning = useHarnessStore((s) => s.reasoning);
  const subjects = useHarnessStore((s) => s.subjects);
  const subjectId = useHarnessStore((s) => s.subjectId);
  const rateLimits = useHarnessStore((s) => s.rateLimits);

  const send = useCallback(
    async (text: string) => {
      const s = store.getState();
      try {
        const newId = await harnessSend(null, s.provider, text, {
          model: s.model,
          reasoningEffort: s.reasoning,
          subjectId: s.subjectId,
        });
        // The rows went in under the new id while we waited; list it, open it,
        // and only then leave — arriving at /chat before the thread exists
        // would land on the empty hero for a beat.
        await s.loadThreads();
        await store.getState().open(newId);
        navigate("/chat");
      } catch (e) {
        // The failure also arrives as an error row through the event path, so
        // there is nothing to say here beyond re-reading the list: the thread
        // may well have been created before the send failed.
        console.error("harness send failed", e);
        await store.getState().loadThreads();
      }
    },
    [store, navigate],
  );

  const onSubject = useCallback((id: number | null) => store.getState().setSubject(id), [store]);
  const onProvider = useCallback((p: Provider) => store.getState().setProvider(p), [store]);
  const onModel = useCallback((m: string | null) => store.getState().setModel(m), [store]);
  const onReasoning = useCallback(
    (level: string | null) => store.getState().setReasoning(level),
    [store],
  );
  const noop = useCallback(() => {}, []);

  return (
    <Composer
      provider={provider}
      model={model}
      reasoning={reasoning}
      providerLocked={false}
      subjects={subjects}
      subjectId={subjectId}
      onSubject={onSubject}
      subjectLocked={false}
      running={false}
      usage={null}
      rateLimits={rateLimits[provider] ?? []}
      onProvider={onProvider}
      onModel={onModel}
      onReasoning={onReasoning}
      onSend={send}
      // Nothing can be in flight from here — the send leaves the page — so
      // stop has nothing to stop.
      onStop={noop}
      autoFocus
    />
  );
}
