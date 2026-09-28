import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { SidebarSimple } from "@phosphor-icons/react";
import { Composer } from "@/components/harness/Composer";
import { ThreadList } from "@/components/harness/ThreadList";
import { ThreadMap } from "@/components/harness/ThreadMap";
import { Timeline, type QuestionActions } from "@/components/harness/Timeline";
import { Button } from "@/components/ui/button";
import { ResizeHandle } from "@/components/ui/ResizeHandle";
import { useResizablePanel } from "@/hooks/useResizablePanel";
import { useStickToBottom } from "@/hooks/useStickToBottom";
import {
  getHarnessRateLimits,
  harnessRefreshRateLimits,
  harnessDeleteThread,
  harnessEditQueued,
  harnessEditResend,
  harnessInterrupt,
  harnessRewind,
  harnessSend,
  harnessUnqueue,
  chatHref,
  chatThreadId,
  parseUsage,
  providerInfo,
} from "@/lib/harness";
import { itemsFor, useHarnessStore } from "@/stores/harnessStore";

const SUGGESTIONS = [
  "What's due this week?",
  "Summarise this week's lecture slides",
  "Find the worked example on Dijkstra",
  "Write a memory about how I like my notes",
];

/** The conversations column. Narrower than ~160 and thread names are all
 *  ellipsis; wider than ~420 and it is eating the timeline it exists to open. */
const LIST = { defaultWidth: 224, minWidth: 160, maxWidth: 420, storageKey: "oculus-chat-list-width" };

/**
 * Chat is a CLI agent — Claude Code or Codex — running from the library's
 * `agents/` folder (`docs/harness.md`). This page is the thread list, the
 * timeline of what the agent said and did, and one composer that sits under
 * the hero on an empty thread and docks at the bottom once there is one.
 *
 * It subscribes slice by slice rather than to the store whole: a turn in
 * flight writes to `live` many times a second, and a page that re-rendered
 * on all of it rebuilt the thread list, the composer and every committed row
 * per token. What actually changes mid-turn — the streaming tail, the running
 * tool's output — subscribes to the store where it is drawn.
 */
export default function ChatPage() {
  const store = useHarnessStore;
  const navigate = useNavigate();
  const here = useLocation();
  // The conversation this tab is showing, read off this tab's own route —
  // not off the store, which every Chat tab shares (`chatHref`).
  const activeId = chatThreadId(here.search);
  // The same id for callbacks that must not be rebuilt every time it changes:
  // the composer, the timeline's question actions and the queue's edits all
  // ask "which thread" at the moment they fire.
  const activeRef = useRef(activeId);
  activeRef.current = activeId;
  const threads = useHarnessStore((s) => s.threads);
  const threadsLoaded = useHarnessStore((s) => s.threadsLoaded);
  const items = useHarnessStore((s) => itemsFor(activeId, s.items));
  const rateLimits = useHarnessStore((s) => s.rateLimits);
  const provider = useHarnessStore((s) => s.provider);
  const model = useHarnessStore((s) => s.model);
  const reasoning = useHarnessStore((s) => s.reasoning);
  const subjects = useHarnessStore((s) => s.subjects);
  const subjectId = useHarnessStore((s) => s.subjectId);
  const running = useHarnessStore((s) => (activeId != null && s.live[activeId]?.running) || false);
  // Words a stop or a rewind handed back to this tab's composer. Local, the way
  // the lecture dock keeps its own: in the store it was one slot for every
  // Chat tab, so a stop in one tab typed its dropped messages into all of them.
  // The counter is what the composer watches — the same text twice is still
  // two restores.
  const [restore, setRestore] = useState<{ text: string; n: number } | null>(null);
  const handBack = useCallback(
    (text: string) => setRestore((r) => ({ text, n: (r?.n ?? 0) + 1 })),
    [],
  );
  // Which threads are busy, as a primitive: selecting the live map itself
  // would put this page back on the token-by-token path the split above
  // exists to leave.
  const runningKey = useHarnessStore((s) =>
    Object.keys(s.live)
      .filter((id) => s.live[Number(id)].running)
      .join(","),
  );
  const runningIds = useMemo(
    () => new Set(runningKey ? runningKey.split(",").map(Number) : []),
    [runningKey],
  );

  const thread = threads.find((t) => t.id === activeId) ?? null;
  const activeProvider = thread?.provider ?? provider;
  const activeModel = thread ? thread.model : model;
  // An open thread shows the scope it was created with; only a new one reads
  // the composer's own selection.
  const activeSubject = thread ? thread.subject_id : subjectId;
  const usage = useMemo(() => parseUsage(thread), [thread]);

  useEffect(() => {
    store.getState().loadThreads();
    store.getState().loadSubjects();
  }, [store]);

  // Put this tab's thread on screen, and say so to the store for as long as it
  // is here — which is what stops the lecture dock releasing a timeline this
  // tab is reading. The thread shown a moment ago lends its rows for the beat
  // the read takes (see `hold`), so a switch never flashes the empty hero.
  const shown = useRef<number | null>(null);
  useEffect(() => {
    if (activeId == null) {
      shown.current = null;
      return;
    }
    void store.getState().hold(activeId, shown.current);
    shown.current = activeId;
    return () => store.getState().unhold(activeId);
  }, [activeId, store]);

  // A route naming a thread that is not in the list, once the list is known:
  // deleted here, deleted from another tab, or a restored tab left on one that
  // has since gone. Back to the empty composer rather than a page that shows
  // nothing and would send into a thread Rust no longer has.
  const missing = threadsLoaded && activeId != null && thread == null;
  useEffect(() => {
    if (missing) navigate(chatHref(null), { replace: true });
  }, [missing, navigate]);

  // The tab wears the conversation's name, the way a project and a lecture tab
  // wear theirs. `tabInfo` titles a tab from its path alone and has no thread
  // list to look one up in, so the name travels in the query beside the id
  // (`chatHref`) and this page is what keeps it current — a thread is born
  // titled with the first line of its first message and renamed once the
  // first exchange is done. Nothing is written until the thread is known, or
  // a restored tab would lose its name for the beat before the list lands.
  // Replace, not push: the back arrow keeps pointing wherever it did, rather
  // than at the same conversation under another name.
  const tabName = thread?.title?.trim() || "";
  useEffect(() => {
    if (activeId != null && thread == null) return;
    const want = chatHref(activeId, tabName);
    if (`${here.pathname}${here.search}` !== want) navigate(want, { replace: true });
  }, [activeId, thread, tabName, here.pathname, here.search, navigate]);

  // Rate limits are per provider account; the stored snapshot draws the bars
  // straight away, before anything is asked of the provider.
  useEffect(() => {
    if (rateLimits[activeProvider]) return;
    getHarnessRateLimits(activeProvider).then((w) => {
      if (w.length) store.setState((s) => ({ rateLimits: { ...s.rateLimits, [activeProvider]: w } }));
    });
  }, [activeProvider, rateLimits, store]);

  // Then replace it with numbers from now. Codex answers a read off its
  // running server; Claude has no equivalent, so there the snapshot stands
  // until the next turn reports. The answer arrives as a `rate_limits` event,
  // which lands in `rateLimits` — so this watches the provider and nothing
  // else, or it would ask again for every answer it got.
  useEffect(() => {
    void harnessRefreshRateLimits(activeProvider).catch(() => {});
  }, [activeProvider]);

  // The questions asked, in order — the rail's landmarks. Derived from the
  // committed rows, so it moves when a turn commits and not per token.
  const markers = useMemo(
    () => items.filter((i) => i.kind === "user").map((i) => ({ id: i.id, text: i.content ?? "" })),
    [items],
  );

  const empty = items.length === 0 && !running;
  const scroll = useStickToBottom(activeId, !empty);
  const list = useResizablePanel(LIST);

  // ⌘⌥B folds the conversations column away, alongside the ⌘B that does the
  // same for the app's sidebar — which is why that one now ignores ⌥.
  //
  // `e.code`, not `e.key`: on macOS ⌥ rewrites the character the key produces,
  // so ⌥B arrives as `∫` and a `key === "b"` test never fires. `code` is the
  // physical key and is the only spelling of this that works.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!e.altKey || !(e.metaKey || e.ctrlKey) || e.code !== "KeyB") return;
      e.preventDefault();
      list.toggle();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [list.toggle]);

  const send = useCallback(
    async (text: string) => {
      const s = store.getState();
      const id = activeRef.current;
      try {
        const newId = await harnessSend(id, activeProvider, text, {
          model: activeModel,
          reasoningEffort: s.reasoning,
          subjectId: s.subjectId,
        });
        if (id == null) {
          // The rows for this thread were written under the new id while we
          // waited; list it, then point *this* tab at it so they show. Replace:
          // the empty composer became this conversation, it is not a page to
          // go back to.
          await s.loadThreads();
          navigate(chatHref(newId), { replace: true });
        }
      } catch (e) {
        // The failure also arrives as an error row through the event path.
        console.error("harness send failed", e);
        if (id == null) await s.loadThreads();
      }
    },
    [store, activeProvider, activeModel, navigate],
  );

  // Opening a thread is a navigation of this tab, so it pushes: the back
  // arrow walks back through the conversations this tab has shown.
  const onOpen = useCallback(
    (id: number) => {
      if (id === activeRef.current) return;
      const t = store.getState().threads.find((x) => x.id === id);
      navigate(chatHref(id, t?.title));
    },
    [store, navigate],
  );
  const onNew = useCallback(
    (subject?: number | null) => {
      if (activeRef.current != null) navigate(chatHref(null));
      // A `+` on a group header means "new thread, in this subject"; the
      // plain New thread button leaves the composer's scope alone.
      if (subject !== undefined) store.getState().setSubject(subject);
    },
    [store, navigate],
  );
  const onDelete = useCallback(
    (id: number) => {
      harnessDeleteThread(id)
        .then(() => store.getState().removed(id))
        // Never silently: a delete that fails leaves the row exactly where it
        // was, which is indistinguishable from a click that never arrived —
        // and that is what a swallowed rejection here cost the last time
        // deleting stopped working.
        .catch((e) => console.error("harness delete failed", e));
    },
    [store],
  );
  const onSubject = useCallback((id: number | null) => store.getState().setSubject(id), [store]);
  const onProvider = useCallback((p: typeof provider) => store.getState().setProvider(p), [store]);
  const onModel = useCallback(
    (m: string | null) => {
      const s = store.getState();
      const open = s.threads.find((t) => t.id === activeRef.current);
      if (open) store.setState({ threads: s.threads.map((t) => (t.id === open.id ? { ...t, model: m } : t)) });
      else s.setModel(m);
    },
    [store],
  );
  const onReasoning = useCallback((r: string | null) => store.getState().setReasoning(r), [store]);
  // Stop means nothing more goes out: the running turn is cut short and
  // anything queued behind it is dropped. Those messages were typed and never
  // sent, so they come back into the composer rather than disappearing.
  const onStop = useCallback(() => {
    const id = activeRef.current;
    if (id == null) return;
    harnessInterrupt(id)
      .then((dropped) => {
        if (dropped.length) handBack(dropped.join("\n\n"));
      })
      .catch(() => {});
  }, [handBack]);
  const onRestored = useCallback(() => setRestore(null), []);

  // Asking the same question differently. The thread rewinds to that row —
  // it and everything after it stop being rows — and the new text goes as the
  // next turn. Rust rewinds the agent's own session to match before it
  // deletes anything (`docs/harness.md`). A provider that cannot do that half
  // (`ProviderInfo.rewind`) is offered none of the three.
  const rewinds = providerInfo(activeProvider)?.rewind ?? false;
  const questions = useMemo((): QuestionActions => {
    // The composer's own send on the open thread: its model, the session's
    // level. What an approved Antigravity permission carries on with.
    const followUp = async (text: string) => {
      const s = store.getState();
      const t = s.threads.find((x) => x.id === activeRef.current);
      if (!t) return;
      await harnessSend(t.id, t.provider, text, { model: t.model, reasoningEffort: s.reasoning });
    };
    if (!rewinds) return { followUp };
    const resend = (itemId: number, text: string) => {
      const s = store.getState();
      const id = activeRef.current;
      if (id == null) return;
      harnessEditResend(id, itemId, text, {
        model: s.threads.find((t) => t.id === id)?.model,
        reasoningEffort: s.reasoning,
      }).catch((e) => console.error("harness edit failed", e));
    };
    return {
      edit: resend,
      // Retry is the same move with the same words: the question is asked
      // again, and the answer it got is no longer part of the thread.
      retry: resend,
      // Rewind sends nothing. The thread goes back to before the question and
      // the words land in the composer, for the student to carry on from.
      rewind: (itemId: number) => {
        const id = activeRef.current;
        if (id == null) return;
        harnessRewind(id, itemId)
          .then(handBack)
          .catch((e) => console.error("harness rewind failed", e));
      },
      followUp,
    };
  }, [store, rewinds, handBack]);

  const pending = useMemo(
    () => ({
      editQueued: (queueId: string, text: string) => {
        const id = activeRef.current;
        if (id != null) harnessEditQueued(id, queueId, text).catch(() => {});
      },
      unqueue: (queueId: string) => {
        const id = activeRef.current;
        if (id != null) harnessUnqueue(id, queueId).catch(() => {});
      },
    }),
    [],
  );

  const composer = (
    <Composer
      provider={activeProvider}
      model={activeModel}
      reasoning={reasoning}
      providerLocked={thread != null}
      subjects={subjects}
      subjectId={activeSubject}
      onSubject={onSubject}
      subjectLocked={thread != null}
      running={running}
      usage={usage}
      rateLimits={rateLimits[activeProvider] ?? []}
      restore={restore}
      onRestored={onRestored}
      onProvider={onProvider}
      onModel={onModel}
      onReasoning={onReasoning}
      onSend={send}
      onStop={onStop}
      autoFocus
    />
  );

  return (
    <div className="flex h-full">
      <ThreadList
        threads={threads}
        subjects={subjects}
        activeId={activeId}
        runningIds={runningIds}
        width={list.width}
        restWidth={list.restWidth}
        collapsed={list.collapsed}
        animate={!list.dragging}
        onToggle={list.toggle}
        onOpen={onOpen}
        onNew={onNew}
        onDelete={onDelete}
      />
      {/* The grip sits *on* the seam rather than in it: `w-1` with a matching
          negative margin either side costs no layout width, so dragging it does
          not shift the timeline by its own thickness. Folded, it stays put at
          the card's left edge — dragging it out is the second way back in. */}
      <ResizeHandle onMouseDown={list.onMouseDown} dragging={list.dragging} className="-mx-0.5" />

      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex h-12 shrink-0 items-center gap-2 border-b border-border-subtle px-6">
          {/* Folded, the panel leaves nothing behind, so the way back in lives
              here — the same trade the app's own sidebar makes with the button
              in the tab strip. */}
          {list.collapsed && (
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label="Show conversations"
              title="Show conversations (⌘⌥B)"
              className="-ml-2 shrink-0 text-muted-foreground"
              onClick={list.toggle}
            >
              <SidebarSimple size={14} />
            </Button>
          )}
          <span className="min-w-0 flex-1 truncate font-display text-[13px] font-semibold text-foreground">
            {thread?.title ?? "Chat"}
          </span>
        </div>

        {empty ? (
          <div className="flex-1 overflow-y-auto px-6">
            <div className="mx-auto flex min-h-full w-full max-w-[760px] flex-col items-center justify-center gap-7 pb-16">
              <div className="flex flex-col items-center gap-4">
                <h1 className="text-display text-foreground">Ask Oculus anything</h1>
                <p className="max-w-md text-center text-xs leading-relaxed text-muted-foreground">
                  A personal university agent with your whole library in front of it — pages, slides, transcripts, Ed threads.
                </p>
              </div>
              <div className="w-full">{composer}</div>
              <div className="flex flex-wrap justify-center gap-2">
                {SUGGESTIONS.map((s) => (
                  <button
                    key={s}
                    type="button"
                    onClick={() => send(s)}
                    className="rounded-full border border-border bg-card px-3 py-1 text-[11.5px] text-muted-foreground transition-colors hover:border-surface-overlay hover:bg-accent hover:text-foreground"
                  >
                    {s}
                  </button>
                ))}
              </div>
            </div>
          </div>
        ) : (
          <>
            <div className="relative min-h-0 flex-1">
              {/* `overflow-x-hidden` is load-bearing, not tidying: `overflow-y: auto`
                  computes the *x* axis to `auto` as well, so one row wider than
                  the column — a long tool path, a table in a reply — would give the
                  whole conversation a horizontal axis and let it slide sideways.
                  The things that genuinely need to scroll across (code blocks,
                  tables) carry their own scroller. */}
              <div ref={scroll.outer} className="h-full overflow-x-hidden overflow-y-auto px-6 py-6">
                <div ref={scroll.inner} className="mx-auto w-full max-w-[760px]">
                  <Timeline
                    items={items}
                    threadId={activeId}
                    running={running}
                    questions={questions}
                    pending={pending}
                  />
                </div>
              </div>
              <ThreadMap scrollRef={scroll.outer} contentRef={scroll.inner} markers={markers} />
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
