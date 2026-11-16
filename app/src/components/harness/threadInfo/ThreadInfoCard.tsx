import { useLayoutEffect, useMemo, useRef } from "react";
import type { Subject } from "@/lib/db";
import type { HarnessItem, HarnessThread, Provider, ThreadUsage } from "@/lib/harness";
import { deriveThreadInfo, type InfoEntry } from "@/lib/harness/threadInfo";
import { cn } from "@/lib/utils";
import { EntryRow } from "./EntryRow";
import { InfoGroup } from "./InfoRow";
import { CARD_WIDTH } from "./layout";
import { ThreadRows } from "./ThreadRows";
import { useLectureTitles } from "./useLectureTitles";
import { WorkRows } from "./WorkRows";

export interface ThreadInfoProps {
  thread: HarnessThread;
  items: HarnessItem[];
  subjects: Subject[];
  provider: Provider;
  model: string | null;
  effort: string | null;
  usage: ThreadUsage | null;
}

/**
 * Thread info's body: what the thread is making (Work), its subject, model
 * and context (unheaded, as one group with Work), then Read and Made. Every
 * row opens its thing beside the chat; nothing here makes a model call. The
 * same card floats in the chat pane or fills the header button's popover.
 */
export function ThreadInfoCard({
  thread,
  items,
  subjects,
  provider,
  model,
  effort,
  usage,
  onHeight,
  onOpened,
  className,
}: ThreadInfoProps & {
  /** Its drawn height, for `threadInfoFloats`. */
  onHeight: (height: number) => void;
  onOpened?: () => void;
  className?: string;
}) {
  const { read, made } = useMemo(
    () => deriveThreadInfo(items, { threadId: thread.id, lectureId: thread.lecture_id }),
    [items, thread.id, thread.lecture_id],
  );
  const lectureIds = useMemo(
    () =>
      read.flatMap((e) => {
        const id = /^lectures\/([^/]+)$/.exec(e.key)?.[1];
        return id ? [id] : [];
      }),
    [read],
  );
  const lectureTitle = useLectureTitles(lectureIds);

  const box = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    const measure = () => onHeight(el.offsetHeight);
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [onHeight]);

  const entry = (e: InfoEntry) => <EntryRow key={e.key} entry={e} lectureTitle={lectureTitle} onOpened={onOpened} />;
  return (
    <div
      ref={box}
      aria-label="Thread info"
      role="region"
      tabIndex={-1}
      style={{ width: CARD_WIDTH }}
      className={cn(
        "flex flex-col rounded-2xl border border-border-subtle bg-popover outline-none p-1.5 text-popover-foreground shadow-lg",
        className,
      )}
    >
      <div className="flex flex-col pb-1">
        <WorkRows thread={thread} onOpened={onOpened} />
        <ThreadRows
          thread={thread}
          subjects={subjects}
          provider={provider}
          model={model}
          effort={effort}
          usage={usage}
          onOpened={onOpened}
        />
      </div>
      <InfoGroup heading="Read" rows={read} render={entry} />
      <InfoGroup heading="Made" rows={made} render={entry} />
    </div>
  );
}
