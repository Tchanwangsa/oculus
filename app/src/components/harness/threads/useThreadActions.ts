import { useCallback, useMemo, useRef } from "react";
import type { PendingActions, QuestionActions } from "@/components/harness/timeline/Timeline";
import {
  commandError,
  harnessEditQueued,
  harnessEditResend,
  harnessForceStop,
  harnessInterrupt,
  harnessReorderQueued,
  harnessResume,
  harnessRewind,
  harnessSend,
  harnessSteerQueued,
  harnessUnqueue,
  threadSelection,
  type Provider,
  type DeliveryMode,
  type Selection,
  type SendOptions,
} from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { usePickStore } from "@/stores/chat/pickStore";

interface ThreadActionsOptions {
  /** The thread the actions act on, read at call time. */
  threadId: () => number | null;
  /** What the composer names: the open thread's selection (a held pick
   *  over it), else the session's. Every send carries it. */
  selection: Selection;
  /** Words a rewind handed back, for the composer. */
  onRestore: (text: string) => void;
  /** A send created a thread; the store's list is already reloaded. */
  onCreated: (id: number) => void | Promise<void>;
}

/**
 * A chat surface's thread actions, shared by the Chat page and the lecture
 * dock. All are stable for the component's life, so `Timeline`'s memoised
 * rows stay memoised.
 */
export function useThreadActions(opts: ThreadActionsOptions) {
  const store = useHarnessStore;
  const ref = useRef(opts);
  ref.current = opts;

  /** `extra` runs after the thread id is read, for options that take a while.
   *  `mode`: how the message enters a running turn (`useSendMode`). Resolves
   *  to Rust's reason when the send was refused, else null, so a caller can
   *  give the words back (the Overview's ask bar). */
  const send = useCallback(
    async (text: string, extra: () => SendOptions | Promise<SendOptions> = () => ({}), mode?: DeliveryMode): Promise<string | null> => {
      const s = store.getState();
      const o = ref.current;
      const id = o.threadId();
      const more = await extra();
      const { provider, model, effort } = o.selection;
      try {
        const newId = await harnessSend(id, provider, text, { model, reasoningEffort: effort, ...more }, mode);
        // The send carried the composer's pick; Rust stored it with the message.
        if (id != null) usePickStore.getState().sent(id);
        if (id == null) {
          // Rows were written under the new id while we waited; open it.
          await s.loadThreads();
          await ref.current.onCreated(newId);
        }
        return null;
      } catch (e) {
        // The failure also arrives as an error row through the event path.
        console.error("harness send failed", e);
        if (id == null) await s.loadThreads();
        return commandError(e);
      }
    },
    [store],
  );

  // Stop names the run the composer showed (`liveRunId`; null only for a
  // turn the CLI started itself) and holds the queue: waiting messages stay
  // listed, and go out again on Resume or with the next message. These three
  // reject with Rust's reason, which the control that sent them shows.
  const onStop = useCallback(async (runId: number | null) => {
    const id = ref.current.threadId();
    if (id != null) await harnessInterrupt(id, runId);
  }, []);
  // Offered once a stop has gone unanswered (`stopOverdue`); it names the run.
  const onForceStop = useCallback(async (runId: number) => {
    const id = ref.current.threadId();
    if (id != null) await harnessForceStop(id, runId);
  }, []);
  const onResume = useCallback(async () => {
    const id = ref.current.threadId();
    if (id != null) await harnessResume(id);
  }, []);

  // An open thread's pick goes to Rust (`pickStore`); a new thread's is the
  // session's, sent with its first message.
  const select = useCallback(
    (change: Partial<Selection>, session: () => void) => {
      const s = store.getState();
      const id = ref.current.threadId();
      const open = s.threads.find((t) => t.id === id);
      if (!open) return session();
      usePickStore.getState().pick(open.id, threadSelection(open, undefined, s.reasoning), change);
    },
    [store],
  );
  const onProvider = useCallback(
    (p: Provider) => select({ provider: p }, () => store.getState().setProvider(p)),
    [store, select],
  );
  const onModel = useCallback(
    (m: string | null) => select({ model: m }, () => store.getState().setModel(m)),
    [store, select],
  );
  const onReasoning = useCallback(
    (r: string | null) => {
      // The session's dial also seeds threads from before levels were stored.
      store.getState().setReasoning(r);
      select({ effort: r }, () => {});
    },
    [store, select],
  );

  // Edit/retry rewind the thread to that row and send anew; Rust rewinds the
  // agent's session first (`docs/harness.md`). The rows offer them only where
  // the question's own run's bridge can rewind (`questionProvider`).
  const questions = useMemo((): QuestionActions => {
    // The composer's send on the open thread, e.g. after an approved permission.
    const followUp = async (text: string) => {
      const { threadId, selection } = ref.current;
      const id = threadId();
      if (id == null) return;
      await harnessSend(id, selection.provider, text, { model: selection.model, reasoningEffort: selection.effort });
      usePickStore.getState().sent(id);
    };
    // The new text goes under what the composer names, which may be another
    // provider's: Rust rewinds the question's session and switches in one commit.
    const resend = (itemId: number, text: string) => {
      const { threadId, selection } = ref.current;
      const id = threadId();
      if (id == null) return;
      harnessEditResend(id, itemId, text, { model: selection.model, reasoningEffort: selection.effort }, selection.provider)
        .then(() => usePickStore.getState().sent(id))
        .catch((e) => console.error("harness edit failed", e));
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
  }, []);

  const pending = useMemo(
    (): PendingActions => ({
      editQueued: async (queueId, text) => {
        const id = ref.current.threadId();
        if (id != null) await harnessEditQueued(id, queueId, text);
      },
      unqueue: async (queueId) => {
        const id = ref.current.threadId();
        if (id != null) await harnessUnqueue(id, queueId);
      },
      reorder: async (order) => {
        const id = ref.current.threadId();
        if (id != null) await harnessReorderQueued(id, order);
      },
      steerQueued: async (queueId) => {
        const id = ref.current.threadId();
        if (id != null) await harnessSteerQueued(id, queueId);
      },
    }),
    [],
  );

  return { send, onStop, onForceStop, onResume, onProvider, onModel, onReasoning, questions, pending };
}
