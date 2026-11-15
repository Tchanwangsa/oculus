import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { Composer } from "@/components/harness/composer/Composer";
import { DraftHero } from "@/components/harness/composer/DraftHero";
import { RecentThreads } from "@/components/harness/threads/RecentThreads";
import { THREAD_LIST_PANEL, ThreadList } from "@/components/harness/threads/ThreadList";
import { ThreadMap } from "@/components/harness/timeline/ThreadMap";
import { JumpToLatest } from "@/components/harness/timeline/JumpToLatest";
import { Timeline } from "@/components/harness/timeline/Timeline";
import { useThreadActions } from "@/components/harness/threads/useThreadActions";
import { PageHeader, PaneTitle, PaneTrail } from "@/components/tabs/PaneHeader";
import { DropOverlay } from "@/components/ui/layout/DropOverlay";
import { SideNavCollapseToggle } from "@/components/ui/layout/SideNav";
import { useSideNavPanel } from "@/hooks/ui/useSideNavPanel";
import { SideNavShell } from "@/components/ui/layout/SideNavShell";
import { useRateLimits } from "@/hooks/agents/useRateLimits";
import { useStickToBottom } from "@/hooks/ui/useStickToBottom";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import {
  harnessDeleteThread,
  chatHref,
  chatScope,
  chatThreadId,
  liveScope,
  parseUsage,
  scopeSubject,
  type ChatScope,
} from "@/lib/harness";
import { itemsFor, useHarnessStore } from "@/stores/chat/harnessStore";
import { draftKey, useDraftStore } from "@/stores/chat/draftStore";

/**
 * Chat with a CLI agent running from the library's `agents/` folder
 * (`docs/harness.md`): the conversations column (`ThreadList`) beside a thread.
 * `?t=<id>` is that thread's timeline; bare `/chat` is a new thread: its
 * headline picks the subject (`DraftHero`), the recent threads
 * (`RecentThreads`) below it. The composer is docked at the bottom of both,
 * and a new thread's will not send until a subject or General is chosen.
 *
 * Subscribes slice by slice: `live` is written many times a second mid-turn,
 * so the streaming parts subscribe where they are drawn, not here.
 */
export default function ChatPage() {
  const store = useHarnessStore;
  const listPanel = useSideNavPanel(THREAD_LIST_PANEL);
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
  const provider = useHarnessStore((s) => s.provider);
  const model = useHarnessStore((s) => s.model);
  const reasoning = useHarnessStore((s) => s.reasoning);
  const subjects = useHarnessStore((s) => s.subjects);
  const subjectsLoaded = useHarnessStore((s) => s.subjectsLoaded);
  // A new thread's scope is its draft's (`draftStore`), shared by every Chat
  // tab as the draft is; unchosen until the student or an entry point picks.
  const newKey = draftKey(null, "chat");
  const storedScope = useDraftStore((s) => s.scopes[newKey]);
  const scope = subjectsLoaded ? liveScope(storedScope, subjects) : null;
  const running = useHarnessStore((s) => (activeId != null && s.live[activeId]?.running) || false);
  // Words a rewind handed back to this tab's composer — local, so a
  // rewind in one Chat tab doesn't type into every tab. `n` makes the same text
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
  // An open thread keeps its own scope; only a new one reads the draft's.
  const activeSubject = thread ? thread.subject_id : scope == null ? null : scopeSubject(scope);
  const usage = useMemo(() => parseUsage(thread), [thread]);
  const rateLimits = useRateLimits(activeProvider);

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

  // An entry point's `?s=` (a group's "+") seeds the new thread's scope once;
  // the route effect below then drops it, so a re-render cannot seed again.
  const seed = activeId == null ? chatScope(here.search) : null;
  useEffect(() => {
    if (seed != null) useDraftStore.getState().setScope(newKey, seed);
  }, [seed, newKey]);

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


  // The rail's landmarks: the questions asked, from committed rows.
  const markers = useMemo(
    () => items.filter((i) => i.kind === "user").map((i) => ({ id: i.id, text: i.content ?? "" })),
    [items],
  );

  const fresh = activeId == null;
  // Only a thread follows its bottom; the recent list is read from the top,
  // so it has a scroller of its own.
  // Borrowed rows belong to the outgoing thread; wait for this thread's
  // history before choosing its initial bottom position.
  const ownRows = items.length === 0 || items[0].thread_id === activeId;
  const scroll = useStickToBottom(activeId, !fresh && ownRows, running, items);
  useScrollFade(scroll.outer, "y", !fresh);
  const recentScroll = useRef<HTMLDivElement>(null);
  useScrollFade(recentScroll, "y", fresh);

  const { send: sendTurn, onStop, onResume, onForceStop, onProvider, onModel, onReasoning, questions, pending } = useThreadActions({
    threadId: () => activeRef.current,
    provider: activeProvider,
    model: activeModel,
    onRestore: handBack,
    // The empty composer became this conversation; replace, it isn't a page
    // to go back to. The next new chat chooses its subject afresh.
    onCreated: (id) => {
      useDraftStore.getState().setScope(newKey, null);
      navigate(chatHref(id), { replace: true });
    },
  });
  const send = useCallback(
    (text: string) =>
      sendTurn(text, () => {
        if (activeRef.current != null) return {};
        const chosen = useDraftStore.getState().scopeFor(newKey, store.getState().subjects);
        // None chosen: the composer holds the send, and Rust refuses one anyway.
        return chosen == null ? {} : { chatScope: chosen };
      }),
    [sendTurn, store, newKey],
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
    (subject?: ChatScope) => {
      // A group header's `+` names its scope; plain New thread keeps the choice.
      const leaving = activeRef.current != null;
      if (subject !== undefined) navigate(chatHref(null, null, { subject }), { replace: !leaving });
      else if (leaving) navigate(chatHref(null));
    },
    [navigate],
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
  const onScope = useCallback((next: ChatScope) => useDraftStore.getState().setScope(newKey, next), [newKey]);
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
      subjectLocked={thread != null}
      unscoped={fresh && scope == null}
      running={running}
      usage={usage}
      rateLimits={rateLimits}
      restore={restore}
      onRestored={onRestored}
      onProvider={onProvider}
      onModel={onModel}
      onReasoning={onReasoning}
      onSend={send}
      onStop={onStop}
      onResume={onResume}
      onForceStop={onForceStop}
      autoFocus
      dropRef={columnRef}
      onDropping={setDropping}
      threadId={activeId}
    />
  );

  return (
    <SideNavShell panel={listPanel} nav={
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
    }>

      <div ref={columnRef} className="relative flex h-full min-w-0 flex-1 flex-col">
        <PageHeader>
          <SideNavCollapseToggle panel={listPanel} />
          <PaneTrail>
            <PaneTitle>{fresh ? "New thread" : thread ? thread.title?.trim() || "Untitled" : ""}</PaneTitle>
          </PaneTrail>
        </PageHeader>

        {fresh ? (
          <div ref={recentScroll} className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-6 py-6">
            <div className="mx-auto w-full max-w-[760px]">
              {subjectsLoaded && <DraftHero subjects={subjects} scope={scope} onScope={onScope} className="pt-10 pb-8" />}
              <RecentThreads threads={threads} subjects={subjects} runningIds={runningIds} onOpen={onOpen} />
            </div>
          </div>
        ) : (
          <div className="relative min-h-0 flex-1">
            {/* `overflow-x-hidden` is load-bearing: `overflow-y: auto` makes x
                `auto` too, so one wide row would scroll the whole thread. */}
            <div ref={scroll.outer} className="h-full overflow-x-hidden overflow-y-auto px-6 pt-6 pb-10">
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
            <JumpToLatest show={scroll.showJump} onClick={scroll.scrollToBottom} />
            <ThreadMap scrollRef={scroll.outer} contentRef={scroll.inner} markers={markers} />
          </div>
        )}
        <div className="shrink-0 px-6 pb-4">
          <div className="mx-auto max-w-[760px]">{composer}</div>
        </div>

        <DropOverlay show={dropping} label="Drop to attach a picture" />
      </div>
    </SideNavShell>
  );
}
