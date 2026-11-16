import { memo, useCallback, useMemo, useState } from "react";
import { ResizeHandle } from "@/components/ui/layout/ResizeHandle";
import { SideNav } from "@/components/ui/layout/SideNav";
import { chatHref, type HarnessThread } from "@/lib/harness";
import type { Subject } from "@/lib/db";
import type { ChatScope } from "@/lib/harness/scope";
import { useCollapsedGroups } from "@/hooks/shell/useCollapsedGroups";
import type { useResizablePanel } from "@/hooks/gestures/useResizablePanel";
import { useStoredState } from "@/hooks/ui/useStoredState";
import { useTabStore } from "@/stores/shell/tabStore";
import { COLLAPSED_KEY, VIEW_KEY, WORKING_FOLD } from "./order";
import { ThreadRow } from "./ThreadRow";
import { useThreadPreviews } from "./useThreadPreviews";
import { ListHeader, readThreadView } from "./list/Header";
import { StatusBody } from "./list/StatusBody";
import { SubjectBody } from "./list/SubjectBody";

export { THREAD_LIST_PANEL } from "./order";

/**
 * Chat's nav column on `SideNav`, the subject page's column: the view switch
 * and new chat on top, then the threads by Status (T3's frame) or by Subject,
 * remembered per device. It takes its width and fold from `panel`
 * (`ChatPage` owns it); ⌘⌥B or the page header's toggle folds it away
 * completely. Titles come from `Harness::name_thread`.
 */
const openInNewTab = (t: HarnessThread) => useTabStore.getState().addTab(chatHref(t.id, t.title));

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
  // Folded sections and groups — not the column's own `collapsed`.
  const { folded, setOpen } = useCollapsedGroups(COLLAPSED_KEY);
  const lines = useThreadPreviews(threads);
  const bySubject = useMemo(() => new Map(subjects.map((s) => [s.id, s])), [subjects]);
  const newChat = useCallback(() => onNew(), [onNew]);

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
      onArm={setConfirming}
      onDelete={onDelete}
    />
  );

  return (
    <>
      <SideNav
        aria-label="Conversations"
        width={panel.width}
        collapsed={collapsed}
        animate={!panel.dragging}
        fill={view === "status"}
        header={
          <ListHeader view={view} onView={setView} fresh={activeId == null} onNew={newChat} collapsed={collapsed} />
        }
      >
        {view === "status" ? (
          <StatusBody
            threads={threads}
            runningIds={runningIds}
            workingOpen={!folded.has(WORKING_FOLD)}
            onWorkingOpen={(open) => setOpen(WORKING_FOLD, open)}
            collapsed={collapsed}
            row={row(true)}
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
