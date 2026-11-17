import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Composer } from "@/components/harness/composer/Composer";
import { useComposerSelection } from "@/components/harness/threads/useComposerSelection";
import { useThreadActions } from "@/components/harness/threads/useThreadActions";
import { useTabId } from "@/components/tabs/TabContext";
import { useRateLimits } from "@/hooks/agents/useRateLimits";
import type { Subject } from "@/lib/db";
import { displayCode } from "@/lib/format/format";
import { chatHref } from "@/lib/harness";
import { warpTo } from "@/lib/ui/warp";
import { draftKey } from "@/stores/chat/draftStore";
import { useHarnessStore } from "@/stores/chat/harnessStore";

const noop = () => {};

/**
 * The subject Overview's ask bar: Chat's composer, `compact`, its subject
 * locked to this one. It names the model and level Chat's new thread would
 * (the session's), and sends through the same `useThreadActions` send, so a
 * send starts a Chat thread filed under the subject. This pane then warps
 * into the thread (`lib/ui/warp.ts`). A refused send gives the words back,
 * with Rust's reason under the bar.
 */
export function OverviewAskBar({ subject }: { subject: Subject }) {
  const navigate = useNavigate();
  const pane = useTabId();
  const subjects = useHarnessStore((s) => s.subjects);
  const { selection } = useComposerSelection(null);
  const rateLimits = useRateLimits(selection.provider);
  const barRef = useRef<HTMLDivElement>(null);
  const [restore, setRestore] = useState<{ text: string; n: number } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const handBack = useCallback((text: string) => setRestore((r) => ({ text, n: (r?.n ?? 0) + 1 })), []);
  const onRestored = useCallback(() => setRestore(null), []);

  // A thread made after the student left the Overview stays in Chat's list;
  // the pane they went to is not pulled away.
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const { send, onProvider, onModel, onReasoning } = useThreadActions({
    threadId: () => null,
    selection,
    onRestore: handBack,
    onCreated: (id) => {
      if (mounted.current) warpTo(barRef.current, pane, id, () => navigate(chatHref(id)));
    },
  });

  const onSend = useCallback(
    (text: string) => {
      setError(null);
      void send(text, () => ({ chatScope: subject.id })).then((refused) => {
        if (refused == null || !mounted.current) return;
        setError(refused);
        handBack(text);
      });
    },
    [send, subject.id, handBack],
  );

  return (
    <div ref={barRef} className="flex flex-col gap-1.5">
      <Composer
        compact
        draftKey={draftKey(null, `overview:${subject.id}`)}
        provider={selection.provider}
        model={selection.model}
        reasoning={selection.effort}
        subjects={subjects}
        subjectId={subject.id}
        subjectLocked
        running={false}
        usage={null}
        rateLimits={rateLimits}
        restore={restore}
        onRestored={onRestored}
        onProvider={onProvider}
        onModel={onModel}
        onReasoning={onReasoning}
        onSend={onSend}
        onStop={noop}
        placeholder={`Ask about ${displayCode(subject.code)}. @ files, / skills`}
      />
      {error && (
        <p role="alert" className="px-0.5 text-[11px] text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}
