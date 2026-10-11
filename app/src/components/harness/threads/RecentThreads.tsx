import { memo, useEffect, useMemo, useState } from "react";
import { CircleNotch, VideoCamera } from "@phosphor-icons/react";
import { ProviderMark } from "@/components/icons/ProviderMark";
import { Badge } from "@/components/ui/badge";
import type { Subject } from "@/lib/db";
import { displayCode, displayName, sqliteUtcToMs } from "@/lib/format/format";
import { chatHref, getThreadPreviews, PROVIDERS, type HarnessThread } from "@/lib/harness";
import { relativeTime } from "@/lib/activity/recents";

/** Rows shown; the nav column lists every thread. */
const SHOWN = 3;

/**
 * Bare `/chat`'s way back into a conversation: the latest threads above the
 * empty composer, each with its subject (or General), age and last reply on
 * one muted line. `threads` is the store's list, newest first.
 */
export const RecentThreads = memo(function RecentThreads({
  threads,
  subjects,
  runningIds,
  onOpen,
}: {
  threads: HarnessThread[];
  subjects: Subject[];
  /** Ids, not the store's live map, which changes on every streamed token. */
  runningIds: Set<number>;
  onOpen: (id: number) => void;
}) {
  const recent = useMemo(() => threads.slice(0, SHOWN), [threads]);
  const bySubject = useMemo(() => new Map(subjects.map((s) => [s.id, s])), [subjects]);
  const [previews, setPreviews] = useState<Map<number, string>>(new Map());

  // Re-read when a listed thread changes, not on every store write.
  const key = recent.map((t) => `${t.id}:${t.updated_at}`).join(",");
  useEffect(() => {
    let live = true;
    getThreadPreviews(recent.map((t) => t.id))
      .then((p) => live && setPreviews(p))
      .catch((e) => console.error("thread previews failed", e));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  if (!recent.length) return null;

  return (
    <section className="flex flex-col">
      <h2 className="px-3 py-1 text-[11px] font-medium text-muted-foreground">Recent</h2>
      <div className="flex flex-col divide-y divide-border-subtle">
        {recent.map((t) => (
          <Row
            key={t.id}
            thread={t}
            subject={t.subject_id == null ? undefined : bySubject.get(t.subject_id)}
            preview={previews.get(t.id)}
            running={runningIds.has(t.id)}
            onOpen={onOpen}
          />
        ))}
      </div>
    </section>
  );
});

function Row({
  thread: t,
  subject,
  preview,
  running,
  onOpen,
}: {
  thread: HarnessThread;
  /** Undefined for a general thread, or one whose subject is gone. */
  subject: Subject | undefined;
  preview: string | undefined;
  running: boolean;
  onOpen: (id: number) => void;
}) {
  const ms = sqliteUtcToMs(t.updated_at);
  return (
    // The pad keeps the hover pill off the hairlines.
    <div className="py-1">
      {/* `data-tab-href`: ⌘-click opens the thread in a new tab. */}
      <button
        type="button"
        data-tab-href={chatHref(t.id, t.title)}
        onClick={() => onOpen(t.id)}
        className="flex w-full min-w-0 items-start gap-3 rounded-lg px-3 py-2 text-left transition-colors hover:bg-accent"
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
            {running ? (
              <CircleNotch size={12} className="shrink-0 animate-spin text-muted-foreground" aria-label="Running" />
            ) : (
              ms != null && (
                <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">{relativeTime(ms)}</span>
              )
            )}
          </span>
          {preview && <span className="truncate text-xs text-muted-foreground">{preview}</span>}
        </span>
      </button>
    </div>
  );
}
