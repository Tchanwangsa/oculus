import { memo, useCallback, useEffect, useMemo, useState } from "react";
import { ResizeHandle } from "@/components/ui/layout/ResizeHandle";
import { SideNav } from "@/components/ui/layout/SideNav";
import { chatHref, type HarnessThread } from "@/lib/harness";
import type { Subject } from "@/lib/db";
import type { ChatScope } from "@/lib/harness/scope";
import { useCollapsedGroups } from "@/hooks/shell/useCollapsedGroups";
import type { useResizablePanel } from "@/hooks/gestures/useResizablePanel";
import { useStoredState } from "@/hooks/ui/useStoredState";
import { useTabStore } from "@/stores/shell/tabStore";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { useCompletedStore } from "@/stores/chat/completedStore";
import { watchThreadsChanged } from "@/stores/chat/threadsChanged";
import { COLLAPSED_KEY, COMPLETED_OPEN, completedOpenKey, VIEW_KEY, WORKING_FOLD } from "./order";
import { ThreadRow } from "./ThreadRow";
import { useThreadPreviews } from "./useThreadPreviews";
import { ListHeader, readThreadView } from "./list/Header";
import { StatusBody } from "./list/StatusBody";
import { SubjectBody } from "./list/SubjectBody";
import { StatusCompleted } from "./completed/StatusCompleted";
import { SubjectCompleted } from "./completed/SubjectCompleted";
import { SearchResults } from "./search/SearchResults";
import { useThreadSearch } from "./search/useThreadSearch";

export { THREAD_LIST_PANEL } from "./order";

/**
 * Chat's nav column on `SideNav`, the subject page's column: search and new
 * chat on top, the view switch under them, then the open threads by Status
 * (T3's frame) or by Subject, remembered per device, each with its
 * Completed folds; typing a search swaps the view for results. It takes its
 * width and fold from `panel` (`ChatPage` owns it); ⌘⌥B or the page
 * header's toggle folds it away completely. Titles come from
 * `Harness::name_thread`.
 */
const openInNewTab = (t: HarnessThread) => useTabStore.getState().addTab(chatHref(t.id, t.title));
const logged = (what: string) => (e: unknown) => console.error(`${what} failed`, e);
const complete = (id: number) => void useCompletedStore.getState().complete(id).catch(logged("complete thread"));
const reopen = (id: number) => void useCompletedStore.getState().reopen(id).catch(logged("reopen thread"));

export const ThreadList = memo(function ThreadList({
  panel,
  threads,
  subjects,
  activeId,
  runningIds,
  onOpen,
  onNew,
  onDelete,
}: {
  panel: ReturnType<typeof useResizablePanel>;
  /** Open threads only (`openThreads`). */
  threads: HarnessThread[];
  subjects: Subject[];
  activeId: number | null;
  /** Ids, not the store's live map, which changes on every streamed token. */
  runningIds: Set<number>;
  onOpen: (id: number) => void;
  /** No argument: the new chat keeps its own choice; otherwise that group's scope. */
  onNew: (scope?: ChatScope) => void;
  onDelete: (id: number) => void;
}) {
  const { collapsed } = panel;
  const [view, setView] = useStoredState(VIEW_KEY, readThreadView);
  const [confirming, setConfirming] = useState<number | null>(null);
  const [query, setQuery] = useState("");
  const found = useThreadSearch(query);
  // Folded sections and groups — not the column's own `collapsed`. Completed
  // folds start closed, so their keys in the set mean open (`order.ts`).
  const { folded, setOpen } = useCollapsedGroups(COLLAPSED_KEY);
  const lines = useThreadPreviews(threads);
  const bySubject = useMemo(() => new Map(subjects.map((s) => [s.id, s])), [subjects]);
  const newChat = useCallback(() => onNew(), [onNew]);

  useEffect(() => {
    useCompletedStore.getState().loadCounts().catch(logged("completed counts read"));
    return watchThreadsChanged();
  }, []);

  // A completed thread is not in the open list: list it first, so the page
  // finds it at once.
  const openThread = useCallback(
    (t: HarnessThread) => {
      if (t.completed_at) useHarnessStore.getState().keep(t);
      onOpen(t.id);
    },
    [onOpen],
  );

  const row = (showSubject: boolean) => (t: HarnessThread) => (
    <ThreadRow
      key={t.id}
      thread={t}
      subject={t.subject_id == null ? undefined : bySubject.get(t.subject_id)}
      showSubject={showSubject}
      preview={lines.messages.get(t.id)}
      question={lines.questions.get(t.id)}
      active={t.id === activeId}
      running={runningIds.has(t.id)}
      confirming={confirming === t.id}
      onOpen={onOpen}
      onNewTab={openInNewTab}
      onComplete={complete}
      onArm={setConfirming}
      onDelete={onDelete}
    />
  );
  const completedProps = { bySubject, activeId, collapsed, onOpen: openThread, onReopen: reopen };

  return (
    <>
      <SideNav
        aria-label="Conversations"
        width={panel.width}
        collapsed={collapsed}
        animate={!panel.dragging}
        fill={view === "status" && !found}
        header={
          <ListHeader
            view={view}
            onView={setView}
            query={query}
            onQuery={setQuery}
            onSearchEnter={() => found?.results[0] && openThread(found.results[0])}
            fresh={activeId == null}
            onNew={newChat}
            collapsed={collapsed}
          />
        }
      >
        {found ? (
          <SearchResults found={found} activeId={activeId} onOpen={openThread} />
        ) : view === "status" ? (
          <StatusBody
            threads={threads}
            runningIds={runningIds}
            workingOpen={!folded.has(WORKING_FOLD)}
            onWorkingOpen={(open) => setOpen(WORKING_FOLD, open)}
            collapsed={collapsed}
            row={row(true)}
            completed={
              <StatusCompleted
                open={folded.has(COMPLETED_OPEN)}
                onOpenFold={(open) => setOpen(COMPLETED_OPEN, !open)}
                {...completedProps}
              />
            }
          />
        ) : (
          <SubjectBody
            threads={threads}
            subjects={subjects}
            activeId={activeId}
            runningIds={runningIds}
            folded={folded}
            onGroupOpen={setOpen}
            collapsed={collapsed}
            onNew={onNew}
            row={row(false)}
            completed={(subjectId, groupKey) => (
              <SubjectCompleted
                subjectId={subjectId}
                open={folded.has(completedOpenKey(groupKey))}
                onOpenFold={(open) => setOpen(completedOpenKey(groupKey), !open)}
                {...completedProps}
              />
            )}
          />
        )}
      </SideNav>
      {/* On the seam: negative margins cost no layout width. Folded away the
          seam is the page's own left edge, where a side panel's handle lives. */}
      {!collapsed && (
        <ResizeHandle
          onMouseDown={panel.onMouseDown}
          dragging={panel.dragging}
          label="Resize conversations"
          className="-mx-0.5"
        />
      )}
    </>
  );
});
