import { useEffect, useRef } from "react";
import { Composer } from "@/components/harness/Composer";
import { ThreadList } from "@/components/harness/ThreadList";
import { Timeline } from "@/components/harness/Timeline";
import {
  getHarnessRateLimits,
  harnessDeleteThread,
  harnessInterrupt,
  harnessSend,
  parseUsage,
} from "@/lib/harness";
import { liveFor, useHarnessStore } from "@/stores/harnessStore";

const SUGGESTIONS = [
  "What's due this week?",
  "Summarise this week's lecture slides",
  "Find the worked example on Dijkstra",
  "Write a memory about how I like my notes",
];

/**
 * Chat is a CLI agent — Claude Code or Codex — running from the library's
 * `agents/` folder (`docs/harness.md`). This page is the thread list, the
 * timeline of what the agent said and did, and one composer that sits under
 * the hero on an empty thread and docks at the bottom once there is one.
 */
export default function ChatPage() {
  const store = useHarnessStore;
  const {
    threads, activeId, items, live, rateLimits, provider, model, reasoning,
    subjects, subjectId,
  } = useHarnessStore();
  const scrollRef = useRef<HTMLDivElement>(null);

  const thread = threads.find((t) => t.id === activeId) ?? null;
  const turn = liveFor(activeId, live);
  const activeProvider = thread?.provider ?? provider;
  const activeModel = thread ? thread.model : model;
  // An open thread shows the scope it was created with; only a new one reads
  // the composer's own selection.
  const activeSubject = thread ? thread.subject_id : subjectId;

  useEffect(() => {
    store.getState().loadThreads();
    store.getState().loadSubjects();
  }, [store]);

  // Rate limits are per provider account; the stored snapshot fills the
  // footer until a live turn reports fresher ones.
  useEffect(() => {
    if (rateLimits[activeProvider]) return;
    getHarnessRateLimits(activeProvider).then((w) => {
      if (w.length) store.setState((s) => ({ rateLimits: { ...s.rateLimits, [activeProvider]: w } }));
    });
  }, [activeProvider, rateLimits, store]);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight });
  }, [items.length, turn.streaming, turn.thinking, turn.running]);

  async function send(text: string) {
    const s = store.getState();
    const id = s.activeId;
    if (id == null) s.beginNew();
    try {
      const newId = await harnessSend(id, activeProvider, text, {
        model: activeModel,
        reasoningEffort: s.reasoning,
        subjectId: s.subjectId,
      });
      if (id == null) {
        // The rows for this thread were written under the new id while we
        // waited; open it so they show, then keep listening live.
        await s.loadThreads();
        await store.getState().open(newId);
      }
    } catch (e) {
      // The failure also arrives as an error row through the event path.
      console.error("harness send failed", e);
      if (id == null) await s.loadThreads();
    }
  }

  const empty = items.length === 0 && !turn.running;

  const composer = (
    <Composer
      provider={activeProvider}
      model={activeModel}
      reasoning={reasoning}
      providerLocked={thread != null}
      subjects={subjects}
      subjectId={activeSubject}
      onSubject={(id) => store.getState().setSubject(id)}
      subjectLocked={thread != null}
      running={turn.running}
      usage={parseUsage(thread)}
      rateLimits={rateLimits[activeProvider] ?? []}
      onProvider={(p) => store.getState().setProvider(p)}
      onModel={(m) => {
        if (thread) store.setState({ threads: threads.map((t) => (t.id === thread.id ? { ...t, model: m } : t)) });
        else store.getState().setModel(m);
      }}
      onReasoning={(r) => store.getState().setReasoning(r)}
      onSend={send}
      onStop={() => activeId != null && harnessInterrupt(activeId).catch(() => {})}
      autoFocus
    />
  );

  return (
    <div className="flex h-full">
      <ThreadList
        threads={threads}
        activeId={activeId}
        live={live}
        onOpen={(id) => store.getState().open(id)}
        onNew={() => store.getState().open(null)}
        onDelete={(id) => harnessDeleteThread(id).then(() => store.getState().removed(id)).catch(() => {})}
      />

      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex h-12 shrink-0 items-center justify-between border-b border-border-subtle px-6">
          <span className="font-display text-[13px] font-semibold text-foreground">
            {thread?.title ?? "Chat"}
          </span>
        </div>

        {empty ? (
          <div className="flex-1 overflow-y-auto px-6">
            <div className="mx-auto flex min-h-full w-full max-w-[760px] flex-col items-center justify-center gap-7 pb-16">
              <div className="flex flex-col items-center gap-4">
                <h1 className="text-display text-foreground">Ask Oculus anything</h1>
                <p className="max-w-md text-center text-[13px] leading-relaxed text-muted-foreground">
                  A coding agent with your whole library in front of it — pages, slides, transcripts,
                  Ed threads — and a memory it keeps between sessions.
                </p>
              </div>
              <div className="w-full">{composer}</div>
              <div className="flex flex-wrap justify-center gap-2">
                {SUGGESTIONS.map((s) => (
                  <button
                    key={s}
                    type="button"
                    onClick={() => send(s)}
                    className="rounded-full border border-border bg-card px-3.5 py-1.5 text-[12px] text-muted-foreground transition-colors hover:border-surface-overlay hover:bg-accent hover:text-foreground"
                  >
                    {s}
                  </button>
                ))}
              </div>
            </div>
          </div>
        ) : (
          <>
            <div ref={scrollRef} className="flex-1 overflow-y-auto px-6 py-6">
              <div className="mx-auto w-full max-w-[760px]">
                <Timeline items={items} live={turn} />
              </div>
            </div>
            <div className="shrink-0 px-6 pb-4 pt-2">
              <div className="mx-auto max-w-[760px]">{composer}</div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
