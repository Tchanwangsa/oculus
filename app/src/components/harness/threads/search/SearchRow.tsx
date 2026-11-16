import { memo } from "react";
import { Books } from "@phosphor-icons/react";
import { ProviderMark } from "@/components/icons/ProviderMark";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode, sqliteUtcToMs } from "@/lib/format/format";
import { shortAge } from "@/lib/activity/recents";
import { chatHref, PROVIDERS, type ThreadSearchResult } from "@/lib/harness";
import { cn } from "@/lib/utils";
import { ROW_META, ROW_PREVIEW, ROW_TITLE, TITLE_READ, TITLE_READ_ACTIVE } from "../rowType";
import { Snippet } from "./Snippet";

/** A search result as a thread row's three lines: subject and age (or
 *  "Completed · age"), title, and the matching line with its hits bold. */
export const SearchRow = memo(function SearchRow({
  result: r,
  active,
  onOpen,
}: {
  result: ThreadSearchResult;
  active: boolean;
  onOpen: (result: ThreadSearchResult) => void;
}) {
  const ms = sqliteUtcToMs(r.updated_at);
  const age = ms == null ? null : shortAge(ms);
  return (
    <button
      type="button"
      data-search-row={r.id}
      data-tab-href={chatHref(r.id, r.title)}
      aria-current={active ? "page" : undefined}
      onClick={() => onOpen(r)}
      className={cn(
        "flex w-full min-w-0 flex-col rounded-lg px-2.5 pt-1.5 pb-[7px] text-left transition-colors",
        active ? "bg-sidebar-item-active" : "hover:bg-sidebar-item-hover",
      )}
    >
      <span className={cn("flex h-[18px] w-full min-w-0 items-center gap-1.5", ROW_META)}>
        <span className="flex shrink-0" title={r.subject_name ?? "Not scoped to a subject"}>
          {r.subject_code ? <SubjectIcon code={r.subject_code} size={13} /> : <Books size={13} />}
        </span>
        <span className="min-w-0 truncate font-medium">{r.subject_code ? displayCode(r.subject_code) : "General"}</span>
        <span className="flex-1" />
        {age && (
          <span className="shrink-0 tabular-nums">{r.completed_at ? `Completed · ${age}` : age}</span>
        )}
      </span>
      <span className={cn("flex h-[21px] w-full min-w-0 items-center", ROW_TITLE)}>
        <span className={cn("min-w-0 flex-1 truncate", active ? TITLE_READ_ACTIVE : TITLE_READ)}>
          {r.title?.trim() || "Untitled"}
        </span>
      </span>
      <span className={cn("flex h-[18px] w-full min-w-0 items-center gap-1.5", ROW_PREVIEW)}>
        <span className="min-w-0 flex-1 truncate">
          <Snippet text={r.snippet} />
        </span>
        <span title={PROVIDERS.find((p) => p.id === r.provider)?.label} className="flex shrink-0">
          <ProviderMark provider={r.provider} className="size-[13px]" />
        </span>
      </span>
    </button>
  );
});
