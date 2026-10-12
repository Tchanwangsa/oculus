import { ArrowSquareOut } from "@phosphor-icons/react";
import { FileRecency } from "@/components/files/FileRecency";
import { RowAction } from "@/components/ui/RowAction";
import { openLinkOut } from "@/lib/files/canvasLinks";
import { filePageHref, openFileSmart } from "@/lib/files/openFile";
import { fmtShortDate } from "@/lib/format/format";
import { threadStatus, threadTitle, type EdThread } from "@/lib/ed/threads";
import { CategoryDot, ThreadKindIcon, ThreadStatusIcon } from "./ThreadIcons";

export const THREAD_COLUMNS = "grid grid-cols-[minmax(0,1fr)_4.5rem] @[600px]:grid-cols-[minmax(0,1fr)_7rem_4.5rem_4rem] @[800px]:grid-cols-[minmax(0,1fr)_10rem_7rem_4.5rem_4rem] items-center gap-4 px-3";

const STATUS_LABEL: Record<string, string> = { resolved: "Answered", unresolved: "Unanswered", unknown: "Unknown" };

export function ThreadRow({ thread }: { thread: EdThread }) {
  const title = threadTitle(thread);
  const status = threadStatus(thread);
  const category = thread.category ?? "Uncategorised";
  return (
    <button
      type="button"
      data-tab-href={filePageHref(thread.file) ?? undefined}
      onClick={() => openFileSmart(thread.file)}
      className={`${THREAD_COLUMNS} group w-full rounded-lg py-2.5 text-left transition-colors hover:bg-surface focus-visible:outline-2 focus-visible:outline-brand focus-visible:-outline-offset-2`}
    >
      <span className="flex min-w-0 items-center gap-3">
        <ThreadKindIcon kind={thread.kind} />
        <span className="min-w-0 flex-1">
          <span title={title} className="block truncate text-[13px] leading-tight font-medium text-foreground">{title}</span>
          {/* Narrow panes fold the hidden columns in under the title. */}
          <span className="mt-0.5 flex min-w-0 items-center gap-1.5 text-[11px] leading-tight text-muted-foreground @[800px]:hidden">
            {thread.category && <CategoryDot color={thread.categoryColor} />}
            <span className="truncate">
              {thread.category}
              {status && <span className="@[600px]:hidden">{thread.category ? " · " : ""}{STATUS_LABEL[status]}</span>}
            </span>
          </span>
        </span>
        {thread.url && (
          <RowAction
            label="Open in Ed"
            onTrigger={(e) => void openLinkOut(thread.url!, e.metaKey)}
            className="shrink-0 opacity-0 transition-opacity will-change-[opacity] hover:text-foreground focus-visible:opacity-100 group-hover:opacity-100"
          >
            <ArrowSquareOut size={14} />
          </RowAction>
        )}
      </span>
      <span title={category} className="hidden min-w-0 items-center gap-2 text-xs text-muted-foreground @[800px]:flex">
        <CategoryDot color={thread.categoryColor} />
        <span className="truncate">{category}</span>
      </span>
      <span className="hidden items-center gap-1.5 text-xs @[600px]:flex">
        <ThreadStatusIcon status={status} />
        <span className={status == null || status === "unknown" ? "text-muted-foreground" : undefined}>
          {status ? STATUS_LABEL[status] : "—"}
        </span>
      </span>
      <span className="text-right text-xs text-muted-foreground tabular-nums">
        {thread.posted ? fmtShortDate(thread.posted) : "—"}
      </span>
      <span className="hidden justify-end @[600px]:flex"><FileRecency file={thread.file} /></span>
    </button>
  );
}
