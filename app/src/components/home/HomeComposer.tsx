import { useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { Composer } from "@/components/harness/Composer";
import { chatHref, harnessSend, type Provider } from "@/lib/harness";
import { useHarnessStore } from "@/stores/harnessStore";
import { draftKey } from "@/stores/draftStore";

/**
 * The Chat composer with the thread pinned at `null`: every send opens a new
 * thread and leaves for `/chat`, so nothing here is provider- or
 * subject-locked. Subjects are loaded by `HomePage` (Continue reads them too).
 * `useNavigate`, not `navigateActive`: each tab has its own memory router.
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
        // List the thread before leaving: a route naming an unlisted thread
        // opens on nothing. The id rides in the route, so only this tab opens it.
        await s.loadThreads();
        navigate(chatHref(newId));
      } catch (e) {
        // The failure also arrives as an error row; the thread may exist.
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
      draftKey={draftKey(null, "home")}
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
      // The send leaves the page, so nothing is ever in flight here.
      onStop={noop}
      autoFocus
    />
  );
}
