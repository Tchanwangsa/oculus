import { memo, useCallback, useMemo, useState } from "react";
import { ResizeHandle } from "@/components/ui/layout/ResizeHandle";
import { SideNav } from "@/components/ui/layout/SideNav";
import { chatHref, type HarnessThread } from "@/lib/harness";
import type { Subject } from "@/lib/db";
import type { useResizablePanel } from "@/hooks/gestures/useResizablePanel";
import { useTabStore } from "@/stores/shell/tabStore";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { ThreadRow } from "./ThreadRow";
import { useThreadPreviews } from "./useThreadPreviews";
import { ListHeader } from "./list/Header";
import { RecentsBody } from "./list/RecentsBody";
import { SearchResults } from "./search/SearchResults";
import { useThreadSearch } from "./search/useThreadSearch";

export { THREAD_LIST_PANEL } from "./order";

/**
 * Chat's nav column on `SideNav`, the subject page's column: search and new
 * chat on top, then every thread newest first under "Recents"; typing a
 * search swaps the list for results. It takes its width and fold from
 * `panel` (`ChatPage` owns it); ⌘⌥B or the page header's toggle folds it
 * away completely. Titles come from `Harness::name_thread`.
 */
const openInNewTab = (t: HarnessThread) => useTabStore.getState().addTab(chatHref(t.id, t.title));
const showMore = () =>
  void useHarnessStore.getState().showMoreThreads().catch((e) => console.error("thread list read failed", e));

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
  onNew: () => void;
  onDelete: (id: number) => void;
}) {
  const { collapsed } = panel;
  const [confirming, setConfirming] = useState<number | null>(null);
  const [query, setQuery] = useState("");
  const found = useThreadSearch(query);
  const more = useHarnessStore((s) => s.moreThreads);
  const lines = useThreadPreviews(threads);
  const bySubject = useMemo(() => new Map(subjects.map((s) => [s.id, s])), [subjects]);

  // A search hit may be older than the threads read: list it first, so the
  // page finds it at once.
  const openThread = useCallback(
    (t: HarnessThread) => {
      useHarnessStore.getState().keep(t);
      onOpen(t.id);
    },
    [onOpen],
  );

  const row = (t: HarnessThread) => (
    <ThreadRow
      key={t.id}
      thread={t}
      subject={t.subject_id == null ? undefined : bySubject.get(t.subject_id)}
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
        header={
          <ListHeader
            query={query}
            onQuery={setQuery}
            onSearchEnter={() => found?.results[0] && openThread(found.results[0])}
            fresh={activeId == null}
            onNew={onNew}
          />
        }
      >
        {found ? (
          <SearchResults found={found} activeId={activeId} onOpen={openThread} />
        ) : (
          <RecentsBody threads={threads} more={more} onMore={showMore} collapsed={collapsed} row={row} />
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
