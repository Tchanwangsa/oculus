import { memo, useCallback, useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { ClockCounterClockwise, NotePencil } from "@phosphor-icons/react";

import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ProviderMark } from "@/components/harness/ProviderMark";
import { Timeline } from "@/components/harness/Timeline";
import { type PickerProvider } from "@/components/harness/ModelPicker";
import { LectureChatComposer } from "@/components/lectures/LectureChatComposer";
import { useStickToBottom } from "@/hooks/useStickToBottom";
import { fmtAgo, sqliteUtcToMs } from "@/lib/format";
import {
  CLAUDE_MODELS,
  PROVIDERS,
  codexAsModels,
  defaultSelection,
  getLectureThreads,
  harnessCodexModels,
  harnessEditQueued,
  harnessEditResend,
  harnessInterrupt,
  harnessRewind,
  harnessSend,
  harnessUnqueue,
  type CodexModel,
  type HarnessThread,
  type Provider,
} from "@/lib/harness";
import { itemsFor, useHarnessStore } from "@/stores/harnessStore";
import { cn } from "@/lib/utils";

/**
 * Which thread each lecture's dock is sitting on.
 *
 * Module-level for the reason `startedAt` is in `useLectureChapters`: the
 * player unmounts every time you switch app tabs, and a conversation that
 * reset to the newest thread — or to an empty composer — under a student who
 * had deliberately gone back to an older one is a worse lie than no memory at
 * all. `null` is a real answer here and not an absent one: it is the student
 * having pressed *New thread*, which a remount must not undo either, so the
 * key's presence is what says the choice has been made.
 */
const dockThread = new Map<string, number | null>();

export interface LectureChatPanelProps {
  lectureId: string;
  /**
   * The playhead, written by the player every `timeupdate`.
   *
   * A ref and not a number: `TranscriptPanel` is memoised against a player
   * that re-renders four times a second, and the whole reason the virtualised
   * transcript stays smooth beside a decoding video is that nothing
   * time-varying reaches it. The chip ticks itself off this once a second, and
   * the send reads it here rather than from a prop that would be a second
   * behind.
   */
  atRef: RefObject<number>;
  /**
   * The moment, built by the player: the transcript of the minute before, the
   * chapter the playhead is in, and the frame grab. Null when there is nothing
   * to say about this second. It never throws — a lecture with no downloaded
   * video has no frame, and a message must still go.
   */
  buildMoment: (at: number) => Promise<string | null>;
}

/**
 * The dock's third reading of a recording: a conversation about it.
 *
 * The Claude Code side panel's shape — the thread's name, a history popover,
 * a new-thread button, the timeline, a composer — scoped to one lecture
 * (`docs/harness.md`). Unlike Chapters and Transcript it is available on every
 * recording: it needs neither a transcript on disk nor a job to have been run,
 * so the tab is never filtered out and the dock is never empty.
 *
 * It owns its thread id rather than the store's `activeId`, which belongs to
 * the Chat page: both can be open at once and on different threads, which is
 * why the store keys rows by thread. `load` puts this one's rows in that map
 * and `release` drops them when the tab or the lecture changes — a dock walked
 * through twenty lectures would otherwise hold twenty timelines, and one can
 * carry 300KB of tool output.
 */
export const LectureChatPanel = memo(function LectureChatPanel({
  lectureId,
  atRef,
  buildMoment,
}: LectureChatPanelProps) {
  const store = useHarnessStore;

  // Resolved from `dockThread` if this lecture has been talked to this
  // session, otherwise from its most recent thread. `resolved` is what keeps
  // the empty hero from flashing over a conversation that is one query away —
  // the same trade `open` makes in the store when it borrows the outgoing
  // thread's rows.
  const [threadId, setThreadId] = useState<number | null>(() => dockThread.get(lectureId) ?? null);
  const [resolved, setResolved] = useState(() => dockThread.has(lectureId));
  /** This lecture's threads, for the history popover and for the title before
   *  the store's own list has been read. */
  const [threads, setThreads] = useState<HarnessThread[]>([]);
  const [codexModels, setCodexModels] = useState<CodexModel[] | null>(null);
  const [moment, setMoment] = useState(true);
  /** Words a stop handed back. Local rather than the store's `restore`, which
   *  is one field with no thread on it: a stop in the dock would otherwise
   *  drop this lecture's half-written question into the Chat page's box. */
  const [restore, setRestore] = useState<{ text: string; n: number } | null>(null);

  const threadIdRef = useRef(threadId);
  threadIdRef.current = threadId;

  const items = useHarnessStore((s) => itemsFor(threadId, s.items));
  const running = useHarnessStore((s) => (threadId != null && s.live[threadId]?.running) || false);
  const storeThread = useHarnessStore((s) => s.threads.find((t) => t.id === threadId) ?? null);
  const provider = useHarnessStore((s) => s.provider);
  const model = useHarnessStore((s) => s.model);
  const reasoning = useHarnessStore((s) => s.reasoning);

  // The store's list is what a running turn patches (`touchThread`), so an
  // open thread's name and status come from there once it has been read; the
  // lecture's own list stands in until then and for a thread older than the
  // hundred most recent in the library.
  const thread = storeThread ?? threads.find((t) => t.id === threadId) ?? null;
  const activeProvider = thread?.provider ?? provider;
  const activeModel = thread ? thread.model : model;

  // The Chat page may never have been opened, and `touchThread` only patches a
  // thread the list holds — a title arriving for one it does not would be
  // dropped. Cheap, and it is the same call the page makes.
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
        // The most recent one, which is the conversation you were having about
        // this recording; a lecture with none opens on the empty composer.
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

  useEffect(() => {
    if (activeProvider !== "codex" || codexModels) return;
    harnessCodexModels().then(setCodexModels).catch(() => setCodexModels([]));
  }, [activeProvider, codexModels]);

  const pickerProviders: PickerProvider[] = PROVIDERS.map((p) =>
    p.id === "claude"
      ? { ...p, models: CLAUDE_MODELS }
      : { ...p, models: codexAsModels(codexModels ?? []), loading: codexModels === null },
  );

  // No turn goes out without a model and a level, so an empty selection —
  // Codex before its CLI has answered — is filled the moment a list exists.
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

  const openThread = useCallback(
    (id: number | null) => {
      dockThread.set(lectureId, id);
      setThreadId(id);
    },
    [lectureId],
  );

  const send = useCallback(
    async (text: string) => {
      const s = store.getState();
      const id = threadIdRef.current;
      // Read the playhead now, not at the last render: the second this says is
      // the second the bubble will carry.
      const at = moment ? Math.max(0, Math.floor(atRef.current)) : null;
      // A frame grab that cannot happen — no downloaded recording — drops the
      // frame line and nothing else. Failing the message over a picture would
      // be the wrong half to lose.
      const context = at == null ? null : await buildMoment(at);
      try {
        const newId = await harnessSend(id, activeProvider, text, {
          model: activeModel,
          reasoningEffort: s.reasoning,
          lectureId,
          context,
          at,
        });
        if (id == null) {
          // The rows were written under the new id while we waited; take it so
          // they show, and put it in the store's list so a title can land on it.
          await s.loadThreads();
          openThread(newId);
          void reloadThreads();
        }
      } catch (e) {
        // The failure also arrives as an error row through the event path.
        console.error("harness send failed", e);
        if (id == null) await s.loadThreads();
      }
    },
    [store, moment, atRef, buildMoment, activeProvider, activeModel, lectureId, openThread, reloadThreads],
  );

  const onStop = useCallback(() => {
    const id = threadIdRef.current;
    if (id == null) return;
    harnessInterrupt(id)
      .then((dropped) => {
        if (!dropped.length) return;
        setRestore((r) => ({ text: dropped.join("\n\n"), n: (r?.n ?? 0) + 1 }));
      })
      .catch(() => {});
  }, []);

  const onProvider = useCallback((p: Provider) => store.getState().setProvider(p), [store]);
  const onModel = useCallback(
    (m: string | null) => {
      const s = store.getState();
      const id = threadIdRef.current;
      const open = s.threads.find((t) => t.id === id);
      // An open thread carries its own model; only a new one reads the
      // session-wide selection the Chat page's composer also sets.
      if (open) store.setState({ threads: s.threads.map((t) => (t.id === open.id ? { ...t, model: m } : t)) });
      else s.setModel(m);
    },
    [store],
  );
  const onReasoning = useCallback((r: string | null) => store.getState().setReasoning(r), [store]);

  // Stable for the life of the panel, so the memoised rows in `Timeline` stay
  // memoised; the thread id is read off the ref at the moment one is used.
  const questions = useMemo(() => {
    const resend = (itemId: number, text: string) => {
      const id = threadIdRef.current;
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
      rewind: (itemId: number) => {
        const id = threadIdRef.current;
        if (id == null) return;
        harnessRewind(id, itemId)
          .then((text) => setRestore((r) => ({ text, n: (r?.n ?? 0) + 1 })))
          .catch((e) => console.error("harness rewind failed", e));
      },
    };
  }, [store]);

  const pending = useMemo(
    () => ({
      editQueued: (queueId: string, text: string) => {
        const id = threadIdRef.current;
        if (id != null) harnessEditQueued(id, queueId, text).catch(() => {});
      },
      unqueue: (queueId: string) => {
        const id = threadIdRef.current;
        if (id != null) harnessUnqueue(id, queueId).catch(() => {});
      },
    }),
    [],
  );

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      {/* The Claude Code panel's top-right pair: where you are, how to get
          back to an earlier conversation, and how to start a new one. */}
      <div className="flex h-8 shrink-0 items-center gap-1 px-2">
        <span className="min-w-0 flex-1 truncate text-[11px] font-medium text-foreground">
          {thread?.title || "New thread"}
        </span>
        {/* `title` rather than a `Tooltip`, the way the thread list's own icon
            buttons do it: a tooltip wrapping a popover trigger is two
            overlays on one element, and the one that stays up is the wrong
            one. */}
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
                  {/* `ThreadList` is the page's column and is built for it —
                      groups, delete confirms, a fold. This list answers one
                      question, so it borrows the mark and nothing else. */}
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

      <div ref={scroll.outer} className="min-h-0 flex-1 overflow-y-auto px-2">
        <div ref={scroll.inner} className="min-w-0 py-1">
          {empty ? (
            // The provider mark and one line. No suggestion chips: the dock is
            // 300px wide and they would be most of it.
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

      <div className="shrink-0 px-2 pb-2 pt-1">
        <LectureChatComposer
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
