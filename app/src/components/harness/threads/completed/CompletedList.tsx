import type { Subject } from "@/lib/db";
import type { CompletedPage, HarnessThread } from "@/lib/harness";
import { SIDE_NAV_FOLDS, SIDE_NAV_ROW, SIDE_NAV_ROW_IDLE } from "@/components/ui/layout/SideNav";
import { cn } from "@/lib/utils";
import { CompletedRow } from "./CompletedRow";

/** The next page's size (`getCompletedThreads`). */
const MORE = 25;

/** A completed fold's rows, then "Show n more" while the read has more. */
export function CompletedList({
  page,
  bySubject,
  showSubject,
  activeId,
  collapsed,
  onOpen,
  onReopen,
  onMore,
}: {
  page: CompletedPage;
  bySubject: Map<number, Subject>;
  showSubject: boolean;
  activeId: number | null;
  collapsed: boolean;
  onOpen: (thread: HarnessThread) => void;
  onReopen: (id: number) => void;
  onMore: () => void;
}) {
  const rest = page.total - page.threads.length;
  return (
    <div className="flex flex-col gap-px">
      {page.threads.map((t) => (
        <CompletedRow
          key={t.id}
          thread={t}
          subject={t.subject_id == null ? undefined : bySubject.get(t.subject_id)}
          showSubject={showSubject}
          active={t.id === activeId}
          collapsed={collapsed}
          onOpen={onOpen}
          onReopen={onReopen}
        />
      ))}
      {page.nextOffset != null && rest > 0 && (
        <button
          type="button"
          tabIndex={collapsed ? -1 : undefined}
          onClick={onMore}
          className={cn(
            SIDE_NAV_ROW,
            SIDE_NAV_ROW_IDLE,
            SIDE_NAV_FOLDS,
            "px-2.5 text-[11.5px] group-data-[collapsed=true]/nav:pointer-events-none",
          )}
        >
          {/* One child: the row's flex gap would split the words. */}
          <span>
            Show <span className="tabular-nums">{Math.min(MORE, rest)}</span> more
          </span>
        </button>
      )}
    </div>
  );
}
