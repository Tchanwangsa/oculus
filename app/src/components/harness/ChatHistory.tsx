import { memo, useEffect, useMemo, useRef, useState } from "react";
import { CaretRight, CircleNotch, Trash, VideoCamera, X } from "@phosphor-icons/react";
import { ProviderMark } from "@/components/harness/ProviderMark";
import { Badge } from "@/components/ui/badge";
import { PillTabs } from "@/components/ui/PillTabs";
import { useCollapsedGroups } from "@/hooks/useCollapsedGroups";
import { useStoredState } from "@/hooks/useStoredState";
import type { Subject } from "@/lib/db";
import { displayCode, displayName, sqliteUtcToMs } from "@/lib/format";
import { chatHref, getThreadPreviews, PROVIDERS, type HarnessThread } from "@/lib/harness";
import { relativeTime } from "@/lib/recents";
import { groupBySubject } from "@/lib/subjectGroups";
import { cn } from "@/lib/utils";

const COLLAPSED_KEY = "oculus-chat-groups-collapsed";
const GROUP_BY_KEY = "oculus-chat-history-group";

/** Threads a group shows at first, and how many each "Show more" adds. */
const PAGE = 10;

type GroupBy = "recent" | "subject" | "provider";

const GROUP_BY_TABS: ReadonlyArray<{ value: GroupBy; label: string }> = [
  { value: "recent", label: "Recent" },
  { value: "subject", label: "Subject" },
  { value: "provider", label: "Provider" },
];

const readGroupBy = (raw: string | null): GroupBy =>
  GROUP_BY_TABS.some((t) => t.value === raw) ? (raw as GroupBy) : "recent";

interface Group {
  key: string;
  label: string;
  title?: string;
  items: HarnessThread[];
}

const DAY_BUCKETS = [
  { key: "date:today", label: "Today", daysBack: 0 },
  { key: "date:yesterday", label: "Yesterday", daysBack: 1 },
  { key: "date:week", label: "Previous 7 days", daysBack: 7 },
  { key: "date:month", label: "Previous 30 days", daysBack: 30 },
] as const;

/** Calendar days back from local midnight, so a DST change can't shift a bucket. */
function byDate(threads: HarnessThread[]): Group[] {
  const starts = DAY_BUCKETS.map((b) => {
    const d = new Date();
    d.setHours(0, 0, 0, 0);
    d.setDate(d.getDate() - b.daysBack);
    return d.getTime();
  });
  const groups: Group[] = [
    ...DAY_BUCKETS.map((b) => ({ key: b.key, label: b.label, items: [] as HarnessThread[] })),
    { key: "date:older", label: "Older", items: [] },
  ];
  for (const t of threads) {
    const ms = sqliteUtcToMs(t.updated_at);
    const at = ms == null ? -1 : starts.findIndex((s) => ms >= s);
    groups[at === -1 ? groups.length - 1 : at].items.push(t);
  }
  return groups.filter((g) => g.items.length);
}

/** First-appearance order, so the most recently used provider leads. */
function byProvider(threads: HarnessThread[]): Group[] {
  const groups = new Map<string, Group>();
  for (const t of threads) {
    let g = groups.get(t.provider);
    if (!g) {
      const label = PROVIDERS.find((p) => p.id === t.provider)?.label ?? t.provider;
      g = { key: `provider:${t.provider}`, label, items: [] };
      groups.set(t.provider, g);
    }
    g.items.push(t);
  }
  return [...groups.values()];
}

/**
 * Bare `/chat`'s list of every thread, grouped by date (Recent), subject or
 * provider, with running threads pinned above in their own group. Each row
 * names its subject, or General. `threads` is the
 * store's list, newest first; titles come from `Harness::name_thread`.
 */
export const ChatHistory = memo(function ChatHistory({
  threads,
  subjects,
  runningIds,
  onOpen,
  onDelete,
}: {
  threads: HarnessThread[];
  subjects: Subject[];
  /** Ids, not the store's live map, which changes on every streamed token. */
  runningIds: Set<number>;
  onOpen: (id: number) => void;
  onDelete: (id: number) => void;
}) {
  const [groupBy, setGroupBy] = useStoredState(GROUP_BY_KEY, readGroupBy);
  const { folded, setOpen: setGroupOpen } = useCollapsedGroups(COLLAPSED_KEY);
  // Per-group page expansion; not persisted.
  const [shown, setShown] = useState<Record<string, number>>({});
  const [confirming, setConfirming] = useState<number | null>(null);
  const [previews, setPreviews] = useState<Map<number, string>>(new Map());

  const bySubject = useMemo(() => new Map(subjects.map((s) => [s.id, s])), [subjects]);

  const groups = useMemo(() => {
    const running = threads.filter((t) => runningIds.has(t.id));
    const rest = running.length ? threads.filter((t) => !runningIds.has(t.id)) : threads;
    const grouped: Group[] =
      groupBy === "provider"
        ? byProvider(rest)
        : groupBy === "subject"
          ? groupBySubject(rest, subjects, { key: "general", label: "General" })
          : byDate(rest);
    return running.length ? [{ key: "running", label: "Running", items: running }, ...grouped] : grouped;
  }, [threads, subjects, runningIds, groupBy]);

  // Re-read when a listed thread changes, not on every store write.
  const key = threads.map((t) => `${t.id}:${t.updated_at}`).join(",");
  useEffect(() => {
    let live = true;
    getThreadPreviews(threads.map((t) => t.id))
      .then((p) => live && setPreviews(p))
      .catch((e) => console.error("thread previews failed", e));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  if (!threads.length) return null;

  const setGroupOpenAndReset = (key: string, open: boolean) => {
    setGroupOpen(key, open);
    // Folding resets the group to one page.
    if (!open) setShown(({ [key]: _dropped, ...rest }) => rest);
  };

  return (
    <div className="flex w-full flex-col gap-3">
      <div className="flex h-7 items-center">
        <PillTabs tabs={GROUP_BY_TABS} value={groupBy} onChange={setGroupBy} />
      </div>

      <div className="flex flex-col gap-4">
        {groups.map((g) => {
          const open = !folded.has(g.key);
          const limit = shown[g.key] ?? PAGE;
          const more = g.items.length - limit;
          return (
            <section key={g.key} className="flex flex-col">
              <button
                type="button"
                title={g.title}
                aria-expanded={open}
                onClick={() => setGroupOpenAndReset(g.key, !open)}
                className="group/head flex w-fit select-none items-center gap-1.5 px-3 py-1 text-left text-[11px] font-medium text-muted-foreground transition-colors hover:text-foreground"
              >
                <span className="truncate">{g.label}</span>
                <span className="tabular-nums opacity-60">{g.items.length}</span>
                <CaretRight
                  size={11}
                  className={cn(
                    "shrink-0 transition-[transform,opacity]",
                    open ? "rotate-90 opacity-0 group-hover/head:opacity-100" : "opacity-60",
                  )}
                />
              </button>
              {open && (
                <div className="flex flex-col divide-y divide-border-subtle">
                  {g.items.slice(0, limit).map((t) => (
                    <Row
                      key={t.id}
                      thread={t}
                      subject={t.subject_id == null ? undefined : bySubject.get(t.subject_id)}
                      preview={previews.get(t.id)}
                      running={runningIds.has(t.id)}
                      confirming={confirming === t.id}
                      onOpen={onOpen}
                      onArm={setConfirming}
                      onDelete={onDelete}
                    />
                  ))}
                  {more > 0 && (
                    <div className="py-1">
                      <button
                        type="button"
                        onClick={() => setShown((prev) => ({ ...prev, [g.key]: limit + PAGE }))}
                        className="w-full rounded-lg px-3 py-1.5 text-left text-[11px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                      >
                        Show <span className="tabular-nums">{Math.min(PAGE, more)}</span> more
                      </button>
                    </div>
                  )}
                </div>
              )}
            </section>
          );
        })}
      </div>
    </div>
  );
});

function Row({
  thread: t,
  subject,
  preview,
  running,
  confirming,
  onOpen,
  onArm,
  onDelete,
}: {
  thread: HarnessThread;
  /** Undefined for a general thread, or one whose subject is gone. */
  subject: Subject | undefined;
  preview: string | undefined;
  running: boolean;
  confirming: boolean;
  onOpen: (id: number) => void;
  /** Arms this row's delete, or disarms with null. */
  onArm: (id: number | null) => void;
  onDelete: (id: number) => void;
}) {
  const ms = sqliteUtcToMs(t.updated_at);
  return (
    // The pad keeps the hover pill off the hairlines.
    <div className="py-1">
      <div className="group/thread flex items-start rounded-lg transition-colors hover:bg-accent">
        {/* `data-tab-href`: ⌘-click opens the thread in a new tab. */}
        <button
          type="button"
          data-tab-href={chatHref(t.id, t.title)}
          onClick={() => onOpen(t.id)}
          className="flex min-w-0 flex-1 items-start gap-3 rounded-lg py-2 pl-3 text-left"
        >
          <span title={PROVIDERS.find((p) => p.id === t.provider)?.label} className="mt-[3px] shrink-0">
            <ProviderMark provider={t.provider} className="size-3.5 text-muted-foreground" />
          </span>
          <span className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="flex min-w-0 items-baseline gap-3">
              <span className="flex min-w-0 flex-1 items-center gap-1.5">
                <span className="min-w-0 truncate text-[13px] text-foreground">{t.title?.trim() || "Untitled"}</span>
                <Badge
                  variant="secondary"
                  title={subject ? displayName(subject.name, subject.code) : "Not scoped to a subject"}
                  className="px-1.5 py-0 text-[10.5px] font-normal"
                >
                  {subject ? displayCode(subject.code) : "General"}
                </Badge>
                {t.lecture_id && (
                  <VideoCamera
                    size={11}
                    className="shrink-0 text-muted-foreground opacity-60"
                    aria-label="Lecture thread"
                  />
                )}
              </span>
              {ms != null && (
                <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">{relativeTime(ms)}</span>
              )}
            </span>
            {preview && <span className="truncate text-xs text-muted-foreground">{preview}</span>}
          </span>
        </button>
        <div className="flex h-9 shrink-0 items-center pl-2 pr-2 text-muted-foreground">
          {running ? (
            <CircleNotch size={12} className="animate-spin" aria-label="Running" />
          ) : confirming ? (
            <ConfirmDelete
              label="Yes"
              onConfirm={() => {
                onArm(null);
                onDelete(t.id);
              }}
              onCancel={() => onArm(null)}
            />
          ) : (
            <button
              type="button"
              aria-label="Delete thread"
              onClick={() => onArm(t.id)}
              className="rounded p-0.5 opacity-0 transition-opacity hover:text-foreground focus-visible:opacity-100 group-hover/thread:opacity-100"
            >
              <Trash size={12} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

/**
 * A row's armed delete. Focused on mount so clicking elsewhere blurs to cancel.
 * Both buttons commit on `mousedown`: WebKit doesn't focus a clicked button, so
 * pressing one blurs the focused confirm first, which unmounts this before a
 * `click` could land.
 */
function ConfirmDelete({
  label,
  onConfirm,
  onCancel,
}: {
  label: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const ref = useRef<HTMLButtonElement>(null);
  // Mount only, so a re-render never steals focus back.
  useEffect(() => ref.current?.focus(), []);
  return (
    <div className="flex shrink-0 items-center gap-0.5">
      <button
        ref={ref}
        type="button"
        onMouseDown={(e) => {
          e.preventDefault();
          onConfirm();
        }}
        onBlur={onCancel}
        /* No onClick for the keyboard to synthesise into, so keys are handled here. */
        onKeyDown={(e) => {
          if (e.key === "Escape") onCancel();
          else if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onConfirm();
          }
        }}
        className="rounded px-1 text-[10.5px] text-destructive hover:bg-destructive/10"
      >
        {label}
      </button>
      <button
        type="button"
        aria-label="Keep thread"
        title="Keep"
        onMouseDown={(e) => {
          e.preventDefault();
          onCancel();
        }}
        className="rounded p-0.5 text-muted-foreground hover:text-foreground"
      >
        <X size={11} weight="bold" />
      </button>
    </div>
  );
}
