import { memo } from "react";
import { Books, VideoCamera } from "@phosphor-icons/react";
import { ProviderMark } from "@/components/icons/ProviderMark";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { SIDE_NAV_FOLDS } from "@/components/ui/layout/SideNav";
import type { Subject } from "@/lib/db";
import { displayCode, displayName, sqliteUtcToMs } from "@/lib/format/format";
import { shortAge } from "@/lib/activity/recents";
import { chatHref, PROVIDERS, type HarnessThread, type ThreadStatus } from "@/lib/harness";
import { activityLine } from "@/lib/harness/activity";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { cn } from "@/lib/utils";
import { ConfirmDelete } from "./parts";
import { RowMenu } from "./RowMenu";
import { ATTENTION, runStartedMs, STATUS_LOOK, StatusPill, threadRowStatus } from "./StatusPill";

/**
 * A thread as three lines (T3's sidebar row): its subject and, at the right,
 * its status or age; its title; its newest message and provider mark. The
 * Subject view, whose group header names the subject, drops line one and
 * moves the right slot onto the title's line. A title is bright and medium
 * while the thread wants the student (`ATTENTION`), muted otherwise. While
 * the column folds, the subject glyph stays longest, with the status dot on
 * its corner.
 */
export const ThreadRow = memo(function ThreadRow({
  thread: t,
  subject,
  showSubject,
  preview,
  question,
  active,
  running,
  confirming,
  onOpen,
  onNewTab,
  onArm,
  onDelete,
}: {
  thread: HarnessThread;
  /** Undefined for a general thread, or one whose subject is gone. */
  subject: Subject | undefined;
  /** Line one, the subject; off where a group header already names it. */
  showSubject: boolean;
  /** The newest message, either side's. */
  preview: string | undefined;
  /** The question it waits on, line three while it does. */
  question: string | undefined;
  active: boolean;
  running: boolean;
  confirming: boolean;
  onOpen: (id: number) => void;
  onNewTab: (thread: HarnessThread) => void;
  /** Arms this row's delete, or disarms with null. */
  onArm: (id: number | null) => void;
  onDelete: (id: number) => void;
}) {
  const title = t.title?.trim() || "Untitled";
  const status = threadRowStatus(t, running);
  const attention = status != null && ATTENTION.has(status);
  const right = <RightSlot thread={t} status={status} />;
  return (
    <div
      data-thread-row={t.id}
      className={cn(
        "group/thread relative rounded-lg transition-colors",
        active ? "bg-sidebar-item-active" : "hover:bg-sidebar-item-hover",
      )}
    >
      {/* The whole row is the hit target; the menu is a sibling, never inside it. */}
      <button
        type="button"
        // ⌘-click opens the thread in a new tab (`app/src/lib/shell/newTabClicks.ts`).
        data-tab-href={chatHref(t.id, t.title)}
        aria-current={active ? "page" : undefined}
        onClick={() => onOpen(t.id)}
        className="flex w-full min-w-0 flex-col gap-px px-2.5 pt-[7px] pb-2 text-left"
      >
        {showSubject && (
          <span className="flex h-[18px] w-full min-w-0 items-center gap-1.5 text-[11.5px] text-muted-foreground">
            <span
              className="relative flex shrink-0"
              title={subject ? displayName(subject.name, subject.code) : "Not scoped to a subject"}
            >
              {subject ? <SubjectIcon code={subject.code} size={13} /> : <Books size={13} />}
              {status && <FoldDot status={status} className="-top-0.5 -right-0.5" />}
            </span>
            <span className={cn("min-w-0 truncate font-medium", SIDE_NAV_FOLDS)}>
              {subject ? displayCode(subject.code) : "General"}
            </span>
            <span className="flex-1" />
            <span className={cn("flex shrink-0", SIDE_NAV_FOLDS)}>{right}</span>
          </span>
        )}
        <span className={cn("flex h-5 w-full min-w-0 items-center gap-1.5 text-[13px]", SIDE_NAV_FOLDS)}>
          <span
            className={cn(
              "min-w-0 flex-1 truncate",
              attention ? "font-medium text-foreground" : active ? "text-foreground/80" : "text-muted-foreground",
            )}
          >
            {title}
          </span>
          {t.lecture_id && (
            <VideoCamera size={12} className="shrink-0 text-muted-foreground opacity-70" aria-label="Lecture thread" />
          )}
          {!showSubject && right}
        </span>
        <span className={cn("flex w-full min-w-0 items-center gap-1.5 text-xs text-muted-foreground/85", SIDE_NAV_FOLDS)}>
          <span className="min-w-0 flex-1 truncate">
            {status === "working" ? (
              <LiveLine threadId={t.id} fallback={preview} />
            ) : status === "input" && question ? (
              question
            ) : (
              preview
            )}
          </span>
          <span title={PROVIDERS.find((p) => p.id === t.provider)?.label} className="flex shrink-0">
            <ProviderMark provider={t.provider} className="size-[13px]" />
          </span>
        </span>
        {!showSubject && status && <FoldDot status={status} className="top-3 left-1" />}
      </button>
      <div
        className={cn(
          "absolute top-1.5 right-1.5 flex items-center",
          !confirming && SIDE_NAV_FOLDS,
          "group-data-[collapsed=true]/nav:pointer-events-none",
        )}
      >
        {confirming ? (
          <div className="rounded-md border border-border-subtle bg-popover px-0.5">
            <ConfirmDelete
              label="Delete"
              onConfirm={() => {
                onArm(null);
                onDelete(t.id);
              }}
              onCancel={() => onArm(null)}
            />
          </div>
        ) : (
          <RowMenu
            title={title}
            onNewTab={() => onNewTab(t)}
            onDelete={() => onArm(t.id)}
            className="opacity-0 group-hover/thread:opacity-100"
          />
        )}
      </div>
    </div>
  );
});

/** The status in its tone, or the thread's age when it has none. */
function RightSlot({ thread, status }: { thread: HarnessThread; status: ThreadStatus | null }) {
  if (status) return <StatusPill status={status} since={status === "working" ? runStartedMs(thread) : null} />;
  const ms = sqliteUtcToMs(thread.updated_at);
  return ms == null ? null : (
    <span className="h-5 shrink-0 text-[11.5px] leading-5 tabular-nums text-muted-foreground/85">{shortAge(ms)}</span>
  );
}

/** What a working run does now, if a view holds its rows; else `fallback`.
 *  The selector returns a string, so streamed tokens re-render nothing. */
function LiveLine({ threadId, fallback }: { threadId: number; fallback: string | undefined }) {
  const line = useHarnessStore((s) => activityLine(s.items[threadId]));
  return <>{line ?? fallback}</>;
}

/** The status in a dot, shown only while the column folds away. */
function FoldDot({ status, className }: { status: ThreadStatus; className: string }) {
  return (
    <span
      aria-hidden
      className={cn(
        "absolute size-1.5 rounded-full opacity-0 transition-opacity will-change-[opacity] duration-150 group-data-[collapsed=true]/nav:opacity-100",
        STATUS_LOOK[status].dot,
        className,
      )}
    />
  );
}
