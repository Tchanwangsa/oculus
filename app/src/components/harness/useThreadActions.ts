import { useCallback, useMemo, useRef } from "react";
import type { PendingActions, QuestionActions } from "@/components/harness/Timeline";
import {
  harnessEditQueued,
  harnessEditResend,
  harnessInterrupt,
  harnessRewind,
  harnessSend,
  harnessUnqueue,
  providerInfo,
  type Provider,
  type SendOptions,
} from "@/lib/harness";
import { useHarnessStore } from "@/stores/harnessStore";

interface ThreadActionsOptions {
  /** The thread the actions act on, read at call time. */
  threadId: () => number | null;
  /** The composer's selection: the open thread's, else the session's. */
  provider: Provider;
  model: string | null;
  /** Words a stop or a rewind handed back, for the composer. */
  onRestore: (text: string) => void;
  /** A send created a thread; the store's list is already reloaded. */
  onCreated: (id: number) => void | Promise<void>;
}

/**
 * A chat surface's thread actions, shared by the Chat page and the lecture
 * dock. Everything but `questions` is stable for the component's life; that
 * changes only with whether the provider can rewind, so `Timeline`'s memoised
 * rows stay memoised.
 */
export function useThreadActions(opts: ThreadActionsOptions) {
  const store = useHarnessStore;
  const ref = useRef(opts);
  ref.current = opts;
  const rewinds = providerInfo(opts.provider)?.rewind ?? false;

  /** `extra` runs after the thread id is read, for options that take a while. */
  const send = useCallback(
    async (text: string, extra: () => SendOptions | Promise<SendOptions> = () => ({})) => {
      const s = store.getState();
      const o = ref.current;
      const id = o.threadId();
      const more = await extra();
      try {
        const newId = await harnessSend(id, o.provider, text, {
          model: o.model,
          reasoningEffort: s.reasoning,
          ...more,
        });
        if (id == null) {
          // Rows were written under the new id while we waited; open it.
          await s.loadThreads();
          await ref.current.onCreated(newId);
        }
      } catch (e) {
        // The failure also arrives as an error row through the event path.
        console.error("harness send failed", e);
        if (id == null) await s.loadThreads();
      }
    },
    [store],
  );

  // Stop cuts the turn and drops the queue; queued text returns to the composer.
  const onStop = useCallback(() => {
    const id = ref.current.threadId();
    if (id == null) return;
    harnessInterrupt(id)
      .then((dropped) => {
        if (dropped.length) ref.current.onRestore(dropped.join("\n\n"));
      })
      .catch(() => {});
  }, []);

  const onProvider = useCallback((p: Provider) => store.getState().setProvider(p), [store]);
  const onModel = useCallback(
    (m: string | null) => {
      const s = store.getState();
      const id = ref.current.threadId();
      const open = s.threads.find((t) => t.id === id);
      // An open thread carries its own model; a new one reads the session's.
      if (open) store.setState({ threads: s.threads.map((t) => (t.id === open.id ? { ...t, model: m } : t)) });
      else s.setModel(m);
    },
    [store],
  );
  const onReasoning = useCallback((r: string | null) => store.getState().setReasoning(r), [store]);

  // Edit/retry rewind the thread to that row and send anew; Rust rewinds the
  // agent's session first (`docs/harness.md`). Offered only where
  // `ProviderInfo.rewind` is true.
  const questions = useMemo((): QuestionActions => {
    // The composer's send on the open thread, e.g. after an approved permission.
    const followUp = async (text: string) => {
      const { threadId, provider, model } = ref.current;
      const id = threadId();
      if (id == null) return;
      await harnessSend(id, provider, text, { model, reasoningEffort: store.getState().reasoning });
    };
    if (!rewinds) return { followUp };
    const resend = (itemId: number, text: string) => {
      const id = ref.current.threadId();
      if (id == null) return;
      const s = store.getState();
      harnessEditResend(id, itemId, text, {
        model: s.threads.find((t) => t.id === id)?.model,
        reasoningEffort: s.reasoning,
      }).catch((e) => console.error("harness edit failed", e));
    };
    return {
      edit: resend,
      retry: resend,
      // Rewind sends nothing; the words land in the composer.
      rewind: (itemId: number) => {
        const id = ref.current.threadId();
        if (id == null) return;
        harnessRewind(id, itemId)
          .then((text) => ref.current.onRestore(text))
          .catch((e) => console.error("harness rewind failed", e));
      },
      followUp,
    };
  }, [store, rewinds]);

  const pending = useMemo(
    (): PendingActions => ({
      editQueued: (queueId, text) => {
        const id = ref.current.threadId();
        if (id != null) harnessEditQueued(id, queueId, text).catch(() => {});
      },
      unqueue: (queueId) => {
        const id = ref.current.threadId();
        if (id != null) harnessUnqueue(id, queueId).catch(() => {});
      },
    }),
    [],
  );

  return { send, onStop, onProvider, onModel, onReasoning, questions, pending };
}
