import { useCallback, useEffect, useRef, useState } from "react";

import { useDefaultModel } from "@/hooks/studio/useDefaultModel";
import { harnessSend } from "@/lib/harness";
import { changeRequestText } from "@/lib/studio/changeRequest";
import {
  componentHistory,
  componentSource,
  restoreComponent,
  type ComponentLevel,
} from "@/lib/studio/components";
import { useHarnessStore } from "@/stores/chat/harnessStore";

/** How long a sent turn may go without showing as running before the page
 *  looks at the component anyway. */
const START_WAIT_MS = 10_000;

export interface ChangeSubject {
  id: number;
  /** The course code, the name of the subject's library folder. */
  code: string;
}

/** What the last change did to a component. */
export interface Applied {
  name: string;
  level: ComponentLevel;
  subject: string | null;
  /** The text changed (or, at a level that had no file, appeared). */
  changed: boolean;
  /** The snapshot Undo writes back; null when there is nothing to go back to. */
  undoId: string | null;
  undone: boolean;
}

interface Sent {
  name: string;
  level: ComponentLevel;
  subject: string | null;
  beforeText: string | null;
  beforeSnapshot: string | null;
}

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/**
 * "Ask for a change": the first request of a page session starts a Chat
 * harness thread, later ones continue it (a new thread when the library's
 * subject changes, since a thread's subject is fixed at creation). When the
 * turn ends the component is read again and `onSettled` fires, so the page
 * can reload its list, source and stage. A change with an earlier snapshot at
 * its level can be undone.
 */
export function useComponentChange(subject: ChangeSubject | null, onSettled: () => void) {
  useDefaultModel();
  const [thread, setThread] = useState<{ id: number; subjectId: number | null } | null>(null);
  const [sent, setSent] = useState<Sent | null>(null);
  const [applied, setApplied] = useState<Applied | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [working, setWorking] = useState<string | null>(null);
  const running = useHarnessStore((s) => (thread ? (s.live[thread.id]?.running ?? false) : false));
  const sawRunning = useRef(false);
  const settledRef = useRef(onSettled);
  settledRef.current = onSettled;

  const send = useCallback(
    async (name: string, level: ComponentLevel, request: string): Promise<boolean> => {
      setError(null);
      setApplied(null);
      setWorking(name);
      const store = useHarnessStore.getState();
      // Only a subject-level component belongs to the subject's library.
      const code = level === "subject" ? (subject?.code ?? null) : null;
      try {
        const [text, history] = await Promise.all([
          componentSource(name, code).catch(() => null),
          componentHistory(name, level, code).catch(() => []),
        ]);
        const open = thread && thread.subjectId === (subject?.id ?? null) ? thread : null;
        const row = open ? store.threads.find((t) => t.id === open.id) : null;
        const id = await harnessSend(
          open?.id ?? null,
          row?.provider ?? store.provider,
          changeRequestText({ name, level, subject: code, request }),
          {
            model: row ? row.model : store.model,
            reasoningEffort: store.reasoning,
            subjectId: subject?.id ?? null,
          },
        );
        if (!open) await store.loadThreads();
        sawRunning.current = false;
        setThread({ id, subjectId: subject?.id ?? null });
        setSent({
          name,
          level,
          subject: code,
          beforeText: text?.level === level ? text.text : null,
          beforeSnapshot: history[0]?.id ?? null,
        });
        return true;
      } catch (e) {
        setError(message(e));
        setWorking(null);
        return false;
      }
    },
    [subject, thread],
  );

  const settle = useCallback(async (s: Sent) => {
    const [text, history] = await Promise.all([
      componentSource(s.name, s.subject).catch(() => null),
      componentHistory(s.name, s.level, s.subject).catch(() => []),
    ]);
    const newest = history[0]?.id ?? null;
    const newerSnapshot = newest !== s.beforeSnapshot;
    setApplied({
      name: s.name,
      level: s.level,
      subject: s.subject,
      changed: newerSnapshot || (text != null && text.level === s.level && text.text !== s.beforeText),
      undoId: newerSnapshot ? newest : null,
      undone: false,
    });
    setSent(null);
    setWorking(null);
    settledRef.current();
  }, []);

  useEffect(() => {
    if (!sent) return;
    if (running) {
      sawRunning.current = true;
      return;
    }
    if (sawRunning.current) {
      sawRunning.current = false;
      void settle(sent);
      return;
    }
    const id = setTimeout(() => void settle(sent), START_WAIT_MS);
    return () => clearTimeout(id);
  }, [sent, running, settle]);

  const undo = useCallback(async () => {
    if (!applied?.undoId) return;
    setError(null);
    try {
      await restoreComponent(applied.name, applied.level, applied.subject, applied.undoId);
      setApplied({ ...applied, undoId: null, undone: true });
      settledRef.current();
    } catch (e) {
      setError(message(e));
    }
  }, [applied]);

  return {
    threadId: thread?.id ?? null,
    /** The component the running turn is changing. */
    working,
    error,
    applied,
    send,
    undo,
  };
}
