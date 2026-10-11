import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { Composer } from "@/components/harness/composer/Composer";
import { RecentThreads } from "@/components/harness/threads/RecentThreads";
import { THREAD_LIST_PANEL, ThreadList } from "@/components/harness/threads/ThreadList";
import { ThreadMap } from "@/components/harness/timeline/ThreadMap";
import { Timeline } from "@/components/harness/timeline/Timeline";
import { useThreadActions } from "@/components/harness/threads/useThreadActions";
import { PaneHeaderRow, PaneTitle, PaneTrail } from "@/components/tabs/PaneHeader";
import { DropOverlay } from "@/components/ui/layout/DropOverlay";
import { SideNavCollapseToggle } from "@/components/ui/layout/SideNav";
import { useResizablePanel } from "@/hooks/gestures/useResizablePanel";
import { useStickToBottom } from "@/hooks/ui/useStickToBottom";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import {
  getHarnessRateLimits,
  harnessRefreshRateLimits,
  harnessDeleteThread,
  chatHref,
  chatThreadId,
  parseUsage,
} from "@/lib/harness";
import { itemsFor, useHarnessStore } from "@/stores/chat/harnessStore";
import { draftKey, useDraftStore } from "@/stores/chat/draftStore";

/**
 * Chat with a CLI agent running from the library's `agents/` folder
 * (`docs/harness.md`): the conversations column (`ThreadList`) beside a thread.
 * `?t=<id>` is that thread's timeline; bare `/chat` is a new thread, the
 * recent threads (`RecentThreads`) filling its empty timeline. The composer is
 * docked at the bottom of both.
 *
 * Subscribes slice by slice: `live` is written many times a second mid-turn,
 * so the streaming parts subscribe where they are drawn, not here.
 */
export default function ChatPage() {
  const store = useHarnessStore;
  const listPanel = useResizablePanel(THREAD_LIST_PANEL);
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
  // Words a stop or a rewind handed back to this tab's composer — local, so a
  // stop in one Chat tab doesn't type into every tab. `n` makes the same text
  // twice two restores.
  const [restore, setRestore] = useState<{ text: string; n: number } | null>(null);
  const handBack = useCallback(
    (text: string) => setRestore((r) => ({ text, n: (r?.n ?? 0) + 1 })),
    [],
  );
  // A primitive, so this page stays off the per-token `live` updates.
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
  // An open thread keeps its own scope; only a new one reads the composer's.
  const activeSubject = thread ? thread.subject_id : subjectId;
  const usage = useMemo(() => parseUsage(thread), [thread]);

  useEffect(() => {
    store.getState().loadThreads();
    store.getState().loadSubjects();
  }, [store]);

  // Hold this tab's thread while it's on screen, so the lecture dock can't
  // release a timeline this tab is reading. The previous thread lends its rows
  // for the read (`hold`), so a switch never flashes an empty timeline.
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

  // A route naming a thread the loaded list doesn't have (deleted here, in
  // another tab, or before a restore) falls back to a new thread.
  const missing = threadsLoaded && activeId != null && thread == null;
  useEffect(() => {
    if (missing) navigate(chatHref(null), { replace: true });
  }, [missing, navigate]);

  // `tabInfo` titles a tab from its path, so the thread's name travels as
  // `?n=` (`chatHref`), updated when the title lands — but not before the
  // thread is known, or a restored tab loses its name. Replace, not push, so
  // the back arrow is unchanged.
  const tabName = thread?.title?.trim() || "";
  useEffect(() => {
    if (activeId != null && thread == null) return;
    const want = chatHref(activeId, tabName);
    if (`${here.pathname}${here.search}` !== want) navigate(want, { replace: true });
  }, [activeId, thread, tabName, here.pathname, here.search, navigate]);

  // Draw the stored snapshot straight away, per provider account.
  useEffect(() => {
    if (rateLimits[activeProvider]) return;
    getHarnessRateLimits(activeProvider).then((w) => {
      if (w.length) store.setState((s) => ({ rateLimits: { ...s.rateLimits, [activeProvider]: w } }));
    });
  }, [activeProvider, rateLimits, store]);

  // Then refresh (Codex only; Claude's arrive with the next turn). The answer
  // lands in `rateLimits` via an event, so depend on the provider alone.
  useEffect(() => {
    void harnessRefreshRateLimits(activeProvider).catch(() => {});
  }, [activeProvider]);

  // The rail's landmarks: the questions asked, from committed rows.
  const markers = useMemo(
    () => items.filter((i) => i.kind === "user").map((i) => ({ id: i.id, text: i.content ?? "" })),
    [items],
  );

  const fresh = activeId == null;
  // Only a thread follows its bottom; the recent list is read from the top,
  // so it has a scroller of its own.
  const scroll = useStickToBottom(activeId, !fresh);
  useScrollFade(scroll.outer, "y", !fresh);
  const recentScroll = useRef<HTMLDivElement>(null);
  useScrollFade(recentScroll, "y", fresh);

  const { send: sendTurn, onStop, onProvider, onModel, onReasoning, questions, pending } = useThreadActions({
    threadId: () => activeRef.current,
    provider: activeProvider,
    model: activeModel,
    onRestore: handBack,
    // The empty composer became this conversation; replace, it isn't a page
    // to go back to.
    onCreated: (id) => navigate(chatHref(id), { replace: true }),
  });
  const send = useCallback(
    (text: string) => sendTurn(text, () => ({ subjectId: store.getState().subjectId })),
    [sendTurn, store],
  );

  // Opening a thread pushes, so back walks this tab's conversations.
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
      // A group header's `+` sets the subject; plain New thread leaves it.
      if (subject !== undefined) store.getState().setSubject(subject);
    },
    [store, navigate],
  );
  const onDelete = useCallback(
    (id: number) => {
      harnessDeleteThread(id)
        .then(() => {
          store.getState().removed(id);
          const drafts = useDraftStore.getState();
          drafts.setDraft(draftKey(id, "chat"), "");
          drafts.setPastes(draftKey(id, "chat"), []);
        })
        // Never swallow: a failed delete looks exactly like a lost click.
        .catch((e) => console.error("harness delete failed", e));
    },
    [store],
  );
  const onSubject = useCallback((id: number | null) => store.getState().setSubject(id), [store]);
  const onRestored = useCallback(() => setRestore(null), []);
  /** Pictures drop anywhere on the conversation, not only on the box. */
  const columnRef = useRef<HTMLDivElement>(null);
  const [dropping, setDropping] = useState(false);

  const composer = (
    <Composer
      draftKey={draftKey(activeId, "chat")}
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
      dropRef={columnRef}
      onDropping={setDropping}
    />
  );

  return (
    <div className="flex h-full">
      <ThreadList
        panel={listPanel}
        threads={threads}
        subjects={subjects}
        activeId={activeId}
        runningIds={runningIds}
        onOpen={onOpen}
        onNew={onNew}
        onDelete={onDelete}
      />

      <div ref={columnRef} className="relative flex h-full min-w-0 flex-1 flex-col">
        <PaneHeaderRow className="flex h-12 shrink-0 items-center gap-2.5 border-b border-border-subtle px-6">
          <SideNavCollapseToggle collapsed={listPanel.collapsed} onToggle={listPanel.toggle} shortcut="⌘⌥B" />
          <PaneTrail>
            <PaneTitle>{fresh ? "New thread" : thread ? thread.title?.trim() || "Untitled" : ""}</PaneTitle>
          </PaneTrail>
        </PaneHeaderRow>

        {fresh ? (
          <div ref={recentScroll} className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-6 py-6">
            <div className="mx-auto w-full max-w-[760px]">
              <RecentThreads threads={threads} subjects={subjects} runningIds={runningIds} onOpen={onOpen} />
            </div>
          </div>
        ) : (
          <div className="relative min-h-0 flex-1">
            {/* `overflow-x-hidden` is load-bearing: `overflow-y: auto` makes x
                `auto` too, so one wide row would scroll the whole thread. */}
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
        )}
        <div className="shrink-0 px-6 pb-4">
          <div className="mx-auto max-w-[760px]">{composer}</div>
        </div>

        <DropOverlay show={dropping} label="Drop to attach a picture" />
      </div>
    </div>
  );
}
