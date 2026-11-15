import { useCallback, useEffect, useRef, useState } from "react";

import { Composer } from "@/components/harness/composer/Composer";
import { LOCK_OTHERS } from "@/components/harness/composer/ModelPicker";
import { JumpToLatest } from "@/components/harness/timeline/JumpToLatest";
import { Timeline } from "@/components/harness/timeline/Timeline";
import { useThreadActions } from "@/components/harness/threads/useThreadActions";
import { useComposerSelection } from "@/components/harness/threads/useComposerSelection";
import { DropOverlay } from "@/components/ui/layout/DropOverlay";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { useStickToBottom } from "@/hooks/ui/useStickToBottom";
import { parseUsage } from "@/lib/harness";
import { draftKey } from "@/stores/chat/draftStore";
import { itemsFor, useHarnessStore } from "@/stores/chat/harnessStore";

/**
 * A video's conversation: the thread's timeline over a composer, as Chat
 * shows it, for the thread it is given. The thread is
 * created with its video (`useNewVideo`), so its provider and model are
 * fixed here. `onTurnEnd` fires when a turn finishes, since the agent may
 * have changed the video's files.
 */
export function ThreadColumn({ threadId, onTurnEnd }: { threadId: number; onTurnEnd: () => void }) {
  const store = useHarnessStore;
  const items = useHarnessStore((s) => itemsFor(threadId, s.items));
  const running = useHarnessStore((s) => s.live[threadId]?.running ?? false);
  const thread = useHarnessStore((s) => s.threads.find((t) => t.id === threadId) ?? null);
  const subjects = useHarnessStore((s) => s.subjects);
  const rateLimits = useHarnessStore((s) => s.rateLimits);
  const [restore, setRestore] = useState<{ text: string; n: number } | null>(null);
  const columnRef = useRef<HTMLDivElement>(null);
  const [dropping, setDropping] = useState(false);

  // A video's thread keeps its agent; model and level picks go to Rust.
  const { selection, refused } = useComposerSelection(thread);
  const activeProvider = selection.provider;

  useEffect(() => {
    void store.getState().loadThreads();
    void store.getState().loadSubjects();
  }, [store]);

  // Hold the rows while this column shows them; release them on the way out
  // unless a Chat tab holds the same thread.
  useEffect(() => {
    void store.getState().hold(threadId);
    return () => {
      store.getState().unhold(threadId);
      store.getState().release(threadId);
    };
  }, [store, threadId]);

  const wasRunning = useRef(running);
  useEffect(() => {
    if (wasRunning.current && !running) onTurnEnd();
    wasRunning.current = running;
  }, [running, onTurnEnd]);

  const scroll = useStickToBottom(threadId, true, running, items);
  useScrollFade(scroll.outer, "y");

  const threadRef = useRef(threadId);
  threadRef.current = threadId;
  const { send, onStop, onResume, onForceStop, onProvider, onModel, onReasoning, questions, pending } = useThreadActions({
    threadId: () => threadRef.current,
    selection,
    onRestore: (text) => setRestore((r) => ({ text, n: (r?.n ?? 0) + 1 })),
    onCreated: () => {},
  });
  const onSend = useCallback((text: string) => void send(text), [send]);

  return (
    <div ref={columnRef} className="relative flex h-full min-w-0 flex-1 flex-col">
      {/* `overflow-x-hidden` is load-bearing: `overflow-y: auto` makes x `auto`
          too, so one wide row would scroll the whole thread. */}
      <div className="relative min-h-0 flex-1">
        <div ref={scroll.outer} className="h-full overflow-x-hidden overflow-y-auto px-6 pt-6 pb-10">
          <div ref={scroll.inner} className="mx-auto w-full max-w-[640px]">
            <Timeline items={items} threadId={threadId} running={running} questions={questions} pending={pending} />
          </div>
        </div>
        <JumpToLatest show={scroll.showJump} onClick={scroll.scrollToBottom} />
      </div>
      <div className="shrink-0 px-6 pb-4">
        <div className="mx-auto max-w-[640px]">
          <Composer
            draftKey={draftKey(threadId, "studio")}
            provider={activeProvider}
            model={selection.model}
            reasoning={selection.effort}
            locked={LOCK_OTHERS}
            refused={refused}
            subjects={subjects}
            subjectId={thread?.subject_id ?? null}
            onSubject={() => {}}
            subjectLocked
            running={running}
            usage={parseUsage(thread)}
            rateLimits={rateLimits[activeProvider] ?? []}
            restore={restore}
            onRestored={() => setRestore(null)}
            onProvider={onProvider}
            onModel={onModel}
            onReasoning={onReasoning}
            onSend={onSend}
            onStop={onStop}
            onResume={onResume}
            onForceStop={onForceStop}
            autoFocus
            dropRef={columnRef}
            onDropping={setDropping}
            placeholder="Ask for a change"
            threadId={threadId}
          />
        </div>
      </div>
      <DropOverlay show={dropping} label="Drop to attach a picture" />
    </div>
  );
}
