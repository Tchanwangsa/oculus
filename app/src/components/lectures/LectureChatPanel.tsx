import { memo, useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { ClockCounterClockwise, NotePencil } from "@phosphor-icons/react";

import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ProviderMark } from "@/components/harness/ProviderMark";
import { Timeline } from "@/components/harness/Timeline";
import { useThreadActions } from "@/components/harness/useThreadActions";
import { LectureChatComposer } from "@/components/lectures/LectureChatComposer";
import { useProviderModels } from "@/hooks/useProviderModels";
import { useStickToBottom } from "@/hooks/useStickToBottom";
import { useScrollFade } from "@/hooks/useScrollFade";
import { fmtAgo, sqliteUtcToMs } from "@/lib/format";
import { defaultSelection, getLectureThreads, type HarnessThread } from "@/lib/harness";
import { itemsFor, useHarnessStore } from "@/stores/harnessStore";
import { draftKey } from "@/stores/draftStore";
import { cn } from "@/lib/utils";

/**
 * Which thread each lecture's dock is on, module-level so it survives the
 * player unmounting on a tab switch. `null` means the student pressed *New
 * thread*; the key's presence is what says a choice was made.
 */
const dockThread = new Map<string, number | null>();

export interface LectureChatPanelProps {
  lectureId: string;
  /** The playhead as a ref, not a number, so nothing time-varying reaches the
   *  memoised `TranscriptPanel`. */
  atRef: RefObject<number>;
  /** The moment for a second (transcript, chapter, frames), or null. Never
   *  throws: a missing frame must not stop the message. */
  buildMoment: (at: number) => Promise<string | null>;
}

/**
 * The dock's Chat tab: a conversation scoped to one lecture (`docs/harness.md`),
 * available on every recording.
 *
 * It owns its thread id, as each Chat tab owns its own in its route.
 * `load`/`release` keep this thread's rows in the store while it is showing;
 * `release` skips a thread a Chat tab holds.
 */
export const LectureChatPanel = memo(function LectureChatPanel({
  lectureId,
  atRef,
  buildMoment,
}: LectureChatPanelProps) {
  const store = useHarnessStore;

  // From `dockThread`, else the lecture's most recent thread. `resolved` keeps
  // the empty hero from flashing before that query lands.
  const [threadId, setThreadId] = useState<number | null>(() => dockThread.get(lectureId) ?? null);
  const [resolved, setResolved] = useState(() => dockThread.has(lectureId));
  /** This lecture's threads, for the history popover and the title. */
  const [threads, setThreads] = useState<HarnessThread[]>([]);
  const [moment, setMoment] = useState(true);
  /** Words a stop handed back. Local, not the store's thread-less `restore`,
   *  which would drop them into the Chat page's composer. */
  const [restore, setRestore] = useState<{ text: string; n: number } | null>(null);

  const threadIdRef = useRef(threadId);
  threadIdRef.current = threadId;

  const items = useHarnessStore((s) => itemsFor(threadId, s.items));
  const running = useHarnessStore((s) => (threadId != null && s.live[threadId]?.running) || false);
  const storeThread = useHarnessStore((s) => s.threads.find((t) => t.id === threadId) ?? null);
  const provider = useHarnessStore((s) => s.provider);
  const model = useHarnessStore((s) => s.model);
  const reasoning = useHarnessStore((s) => s.reasoning);

  // The store's list is what a running turn patches (`touchThread`); the
  // lecture's own list stands in until it is read, or for an older thread.
  const thread = storeThread ?? threads.find((t) => t.id === threadId) ?? null;
  const activeProvider = thread?.provider ?? provider;
  const activeModel = thread ? thread.model : model;

  // `touchThread` only patches a thread the store's list holds, and the Chat
  // page may never have loaded it.
  useEffect(() => {
    store.getState().loadThreads();
  }, [store]);

  const reloadThreads = useCallback(
    () => getLectureThreads(lectureId).then(setThreads).catch(() => {}),
    [lectureId],
  );

  useEffect(() => {
    if (dockThread.has(lectureId)) {
      setThreadId(dockThread.get(lectureId) ?? null);
      setResolved(true);
      void reloadThreads();
      return;
    }
    let stale = false;
    setResolved(false);
    getLectureThreads(lectureId)
      .then((list) => {
        if (stale) return;
        setThreads(list);
        const id = list[0]?.id ?? null;
        dockThread.set(lectureId, id);
        setThreadId(id);
        setResolved(true);
      })
      .catch(() => {
        if (!stale) setResolved(true);
      });
    return () => {
      stale = true;
    };
  }, [lectureId, reloadThreads]);

  // Hold the thread's rows only while this panel is the one showing them.
  useEffect(() => {
    if (threadId == null) return;
    void store.getState().load(threadId);
    return () => {
      store.getState().release(threadId);
    };
  }, [store, threadId]);

  const { providers: pickerProviders } = useProviderModels(activeProvider);

  // No turn goes out without a model and a level: fill an empty selection once
  // the catalogue arrives.
  const active = pickerProviders.find((p) => p.id === activeProvider);
  useEffect(() => {
    if (model || !active || active.loading || active.models.length === 0) return;
    const pick = defaultSelection(active.models);
    if (!pick.model) return;
    const s = store.getState();
    s.setModel(pick.model);
    s.setReasoning(pick.reasoning);
  }, [model, active, store]);

  const empty = items.length === 0 && !running;
  const scroll = useStickToBottom(threadId, !empty);
  useScrollFade(scroll.outer, "y", !empty);

  const openThread = useCallback(
    (id: number | null) => {
      dockThread.set(lectureId, id);
      setThreadId(id);
    },
    [lectureId],
  );

  const { send: sendTurn, onStop, onProvider, onModel, onReasoning, questions, pending } = useThreadActions({
    threadId: () => threadIdRef.current,
    provider: activeProvider,
    model: activeModel,
    onRestore: (text) => setRestore((r) => ({ text, n: (r?.n ?? 0) + 1 })),
    onCreated: (id) => {
      openThread(id);
      void reloadThreads();
    },
  });
  // No moment on follow-ups: an approval is not a question about the recording.
  const send = useCallback(
    (text: string) =>
      sendTurn(text, async () => {
        // Read the playhead now: this is the second the bubble will carry.
        const at = moment ? Math.max(0, Math.floor(atRef.current)) : null;
        const context = at == null ? null : await buildMoment(at);
        return { lectureId, context, at };
      }),
    [sendTurn, moment, atRef, buildMoment, lectureId],
  );

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      <div className="flex h-8 shrink-0 items-center gap-1 px-2">
        <span className="min-w-0 flex-1 truncate text-[11px] font-medium text-foreground">
          {thread?.title || "New thread"}
        </span>
        {/* `title`, not `Tooltip`: a tooltip on a popover trigger leaves the
            wrong overlay up. */}
        <Popover onOpenChange={(open) => open && void reloadThreads()}>
          <PopoverTrigger asChild>
            <button
              type="button"
              aria-label="Earlier conversations"
              title="Earlier conversations about this lecture"
              className="flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            >
              <ClockCounterClockwise size={13} />
            </button>
          </PopoverTrigger>
          <PopoverContent align="end" className="max-h-72 w-64 overflow-y-auto p-1">
            {threads.length === 0 ? (
              <p className="px-2 py-1.5 text-[11px] text-muted-foreground">
                No conversations about this lecture yet.
              </p>
            ) : (
              threads.map((t) => (
                <button
                  key={t.id}
                  type="button"
                  onClick={() => openThread(t.id)}
                  className={cn(
                    "flex w-full cursor-pointer items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[11.5px] transition-colors",
                    t.id === threadId
                      ? "bg-accent text-foreground"
                      : "text-muted-foreground hover:bg-accent hover:text-foreground",
                  )}
                >
                  <ProviderMark provider={t.provider} className="size-3.5 shrink-0 opacity-70" />
                  <span className="min-w-0 flex-1 truncate">{t.title || "Untitled"}</span>
                  <span className="shrink-0 text-[10px] tabular-nums opacity-60">
                    {fmtAgo(sqliteUtcToMs(t.updated_at))}
                  </span>
                </button>
              ))
            )}
          </PopoverContent>
        </Popover>
        <button
          type="button"
          aria-label="New thread"
          title="New thread about this lecture"
          onClick={() => openThread(null)}
          className="flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          <NotePencil size={13} />
        </button>
      </div>

      {/* `overflow-x-hidden`: `overflow-y: auto` makes x `auto` too, and one
          wide row would scroll the whole column. See `RowShell`. */}
      <div ref={scroll.outer} className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-2">
        <div ref={scroll.inner} className="min-w-0 py-1">
          {empty ? (
            // No suggestion chips: the dock is too narrow.
            resolved && (
              <div className="flex flex-col items-center gap-2 px-3 py-8 text-center">
                <ProviderMark provider={activeProvider} className="size-5 text-muted-foreground opacity-60" />
                <p className="text-[11px] leading-relaxed text-muted-foreground">
                  Ask about this lecture. Each message can carry the moment you are at.
                </p>
              </div>
            )
          ) : (
            <Timeline
              items={items}
              threadId={threadId}
              running={running}
              questions={questions}
              pending={pending}
            />
          )}
        </div>
      </div>

      <div className="shrink-0 px-2 pb-2">
        <LectureChatComposer
          draftKey={draftKey(threadId, `lecture:${lectureId}`)}
          providers={pickerProviders}
          provider={activeProvider}
          providerLocked={thread != null}
          model={activeModel}
          reasoning={reasoning}
          running={running}
          atRef={atRef}
          moment={moment}
          onMoment={setMoment}
          restore={restore}
          onProvider={onProvider}
          onModel={onModel}
          onReasoning={onReasoning}
          onSend={send}
          onStop={onStop}
        />
      </div>
    </div>
  );
});
