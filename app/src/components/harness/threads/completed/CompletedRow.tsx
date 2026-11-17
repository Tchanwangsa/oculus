import { memo } from "react";
import { ArrowCounterClockwise, Books } from "@phosphor-icons/react";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { SIDE_NAV_FOLDS } from "@/components/ui/layout/SideNav";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { Subject } from "@/lib/db";
import { sqliteUtcToMs } from "@/lib/format/format";
import { shortAge } from "@/lib/activity/recents";
import { chatHref, type HarnessThread } from "@/lib/harness";
import { cn } from "@/lib/utils";

/**
 * A completed thread as one grey line (T3): its subject's glyph in grey, its
 * title and age. Hover swaps the age for a Reopen icon, and the title fades
 * out under it as a tab's title does under its ×. The row itself opens the
 * chat, which stays completed until a message or Reopen.
 */
export const CompletedRow = memo(function CompletedRow({
  thread: t,
  subject,
  showSubject,
  active,
  collapsed,
  onOpen,
  onReopen,
}: {
  thread: HarnessThread;
  subject: Subject | undefined;
  /** The glyph; off in the Subject view, under its subject's header. */
  showSubject: boolean;
  active: boolean;
  collapsed: boolean;
  onOpen: (thread: HarnessThread) => void;
  onReopen: (id: number) => void;
}) {
  const title = t.title?.trim() || "Untitled";
  const ms = sqliteUtcToMs(t.updated_at);
  return (
    <div
      data-completed-row={t.id}
      className={cn(
        "group/done relative rounded-lg text-muted-foreground transition-colors",
        active ? "bg-sidebar-item-active text-foreground/80" : "hover:bg-sidebar-item-hover",
      )}
    >
      <button
        type="button"
        data-tab-href={chatHref(t.id, t.title)}
        aria-current={active ? "page" : undefined}
        tabIndex={collapsed ? -1 : undefined}
        onClick={() => onOpen(t)}
        className="flex h-[30px] w-full min-w-0 items-center gap-2 px-2.5 text-left text-[12.5px]"
      >
        {showSubject && (
          <span className="flex shrink-0 opacity-70">
            {subject ? <SubjectIcon code={subject.code} size={13} mono /> : <Books size={13} />}
          </span>
        )}
        <span
          className={cn(
            "min-w-0 flex-1 overflow-hidden whitespace-nowrap [text-overflow:clip]",
            "[mask-image:linear-gradient(to_right,#000_calc(100%_-_22px),transparent)]",
            "group-hover/done:[mask-image:linear-gradient(to_right,#000_calc(100%_-_36px),transparent_calc(100%_-_16px))]",
            "group-has-[[data-reopen]:focus-visible]/done:[mask-image:linear-gradient(to_right,#000_calc(100%_-_36px),transparent_calc(100%_-_16px))]",
            SIDE_NAV_FOLDS,
          )}
        >
          {title}
        </span>
        {ms != null && (
          <span
            className={cn(
              "shrink-0 text-[11.5px] tabular-nums group-hover/done:hidden group-has-[[data-reopen]:focus-visible]/done:hidden",
              SIDE_NAV_FOLDS,
            )}
          >
            {shortAge(ms)}
          </span>
        )}
      </button>
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            data-reopen
            aria-label={`Reopen ${title}`}
            tabIndex={collapsed ? -1 : undefined}
            onClick={() => onReopen(t.id)}
            className={cn(
              "absolute top-1/2 right-1.5 flex size-[22px] -translate-y-1/2 items-center justify-center rounded-[5px] text-muted-foreground opacity-0 outline-none transition-opacity will-change-[opacity] hover:bg-sidebar-item-active hover:text-foreground focus-visible:opacity-100 group-hover/done:opacity-100",
              "group-data-[collapsed=true]/nav:pointer-events-none",
            )}
          >
            <ArrowCounterClockwise size={12} weight="bold" />
          </button>
        </TooltipTrigger>
        <TooltipContent side="right">Reopen</TooltipContent>
      </Tooltip>
    </div>
  );
});
