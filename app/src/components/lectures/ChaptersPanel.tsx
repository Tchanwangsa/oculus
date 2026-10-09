import { memo, useEffect, useRef, type RefObject } from "react";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import type { Chapter } from "@/lib/db";
import { InlineMd } from "@/components/markdown/MdComponents";
import { EntryProgress } from "@/components/lectures/EntryProgress";
import { RegenerateRow, RunStatus } from "@/components/lectures/RunStatus";
import { PanelEmpty } from "@/components/media/MediaDock";
import { CHAPTER_PHASE_LABEL, chapterEnds, type ChapterRunProgress } from "@/lib/lectures";
import { fmtClockSecs } from "@/lib/lectures/media";
import type { ChapterStatus } from "@/hooks/lectures/useLectureChapters";

/** A chapter's length to the minute — chapters are minutes long, and seconds
 *  would push the title onto another line in a narrow dock. */
function fmtSpan(seconds: number): string {
  const m = Math.round(seconds / 60);
  return m < 1 ? "<1m" : `${m}m`;
}

export interface ChaptersPanelProps {
  chapters: Chapter[];
  /** Which chapter the playhead is in; -1 before the first one starts. */
  activeIdx: number;
  /** The playhead, as a ref so this memoised panel doesn't re-render with the
   *  player — see `atRef` in `LecturePlayer.tsx`. */
  atRef: RefObject<number>;
  /** Where the last chapter ends: the element's duration where loaded. */
  duration: number;
  status: ChapterStatus;
  /** `chapter_error`, shown verbatim: it is the agent's own failure. */
  error: string | null;
  /** When this session claimed the run, for the elapsed clock. */
  since: number | null;
  /** What the run is doing right now, or null before it has said. */
  progress: ChapterRunProgress | null;
  /** A run is in flight (Rust refuses a second one). */
  busy: boolean;
  /** Chaptering watches the recording, so it needs the file on disk. */
  downloaded: boolean;
  onSeek: (seconds: number) => void;
  onFind: (force: boolean) => void;
  /** The end job's error (`lib/lectures/end.ts`), or null unless it failed. */
  endError: string | null;
  onRetryEnd: () => void;
}

/**
 * The chapter list. A handful of rows, so it follows playback with a plain
 * `scrollIntoView` rather than the transcript's `FollowList`. Chapters are
 * agent-written, so the only affordances are Find chapters and Regenerate.
 */
export const ChaptersPanel = memo(function ChaptersPanel({
  chapters,
  activeIdx,
  atRef,
  duration,
  status,
  error,
  since,
  progress,
  busy,
  downloaded,
  onSeek,
  onFind,
  endError,
  onRetryEnd,
}: ChaptersPanelProps) {
  const activeRef = useRef<HTMLButtonElement | null>(null);

  // `nearest`, so a card already on screen doesn't jump.
  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: "nearest" });
  }, [activeIdx]);

  const endFailed = endError != null && <EndFailed error={endError} onRetry={onRetryEnd} />;

  if (chapters.length === 0) {
    return (
      <>
        <div className="flex-1 min-h-0 overflow-y-auto">
          {!downloaded ? (
            <PanelEmpty>
              <p>Chapters are found by watching the recording.</p>
              <p className="text-muted-foreground">Download it first.</p>
            </PanelEmpty>
          ) : status === "running" ? (
            <PanelEmpty>
              <ChapterRun since={since} progress={progress} />
            </PanelEmpty>
          ) : status === "error" ? (
            <PanelEmpty>
              <Failed error={error} busy={busy} onRetry={() => onFind(true)} />
            </PanelEmpty>
          ) : (
            <PanelEmpty>
              <Button size="xs" disabled={busy} onClick={() => onFind(false)}>
                Find chapters
              </Button>
              <p className="text-muted-foreground">Takes 8–11 minutes.</p>
            </PanelEmpty>
          )}
        </div>
        {endFailed}
      </>
    );
  }

  const starts = chapters.map((c) => c.start_seconds);
  const ends = chapterEnds(starts, duration);

  return (
    <>
      <div className="flex-1 min-h-0 overflow-y-auto px-1.5 py-2">
        {chapters.map((c, i) => {
          const active = i === activeIdx;
          return (
            <button
              key={c.idx}
              ref={active ? activeRef : undefined}
              onClick={() => onSeek(c.start_seconds)}
              className={cn(
                "relative mb-0.5 flex w-full items-start gap-2 overflow-hidden rounded px-2 py-1.5 text-left transition-colors",
                active
                  ? "bg-brand/12"
                  : "text-muted-foreground hover:bg-surface hover:text-foreground",
              )}
            >
              {active && (
                <EntryProgress atRef={atRef} start={c.start_seconds} end={ends[i]} />
              )}
              <span
                className={cn(
                  "w-10 shrink-0 pt-px text-[10px] tabular-nums",
                  active ? "text-brand/70" : "opacity-60",
                )}
              >
                {fmtClockSecs(c.start_seconds)}
              </span>
              <span className="min-w-0 flex-1">
                <span className="flex items-baseline gap-2">
                  <span
                    className={cn(
                      "min-w-0 flex-1 text-[11px] font-medium leading-snug",
                      active && "text-brand",
                    )}
                  >
                    {c.title}
                  </span>
                  <span
                    className={cn(
                      "shrink-0 text-[10px] tabular-nums",
                      active ? "text-brand/70" : "opacity-60",
                    )}
                  >
                    {fmtSpan(ends[i] - c.start_seconds)}
                  </span>
                </span>
                {/* Inline-only: a `<p>` in a `<button>` closes the button
                    early in WebKit. */}
                {active && (
                  <InlineMd
                    text={c.summary}
                    className="mt-1 block text-[11px] leading-relaxed text-muted-foreground"
                  />
                )}
              </span>
            </button>
          );
        })}
      </div>

      <div className="shrink-0 border-t border-border px-2 py-1.5">
        {status === "running" ? (
          <ChapterRun since={since} progress={progress} compact />
        ) : (
          <RegenerateRow
            disabled={busy}
            error={status === "error" ? error : null}
            onClick={() => onFind(true)}
          />
        )}
      </div>
      {endFailed}
    </>
  );
});

/** The end job failed: Done and Up Next fall back to the file's end until a
 *  Retry finds it. One line, under the chapters, since this tab is where
 *  lecture jobs show. */
function EndFailed({ error, onRetry }: { error: string; onRetry: () => void }) {
  const text = `Couldn't find where this lecture ends — ${error || "the run failed"}`;
  return (
    <div className="flex shrink-0 items-center gap-2 border-t border-border px-2 py-1.5">
      <span className="min-w-0 flex-1 truncate text-[10px] text-destructive" title={text}>
        {text}
      </span>
      <Button size="xs" variant="ghost" onClick={onRetry} className="-mr-1 text-muted-foreground">
        Retry
      </Button>
    </div>
  );
}

function ChapterRun({
  since,
  progress,
  compact,
}: {
  since: number | null;
  progress: ChapterRunProgress | null;
  compact?: boolean;
}) {
  return (
    <RunStatus
      since={since}
      label={progress ? CHAPTER_PHASE_LABEL[progress.phase] : "Finding chapters"}
      progress={progress}
      note="Usually 8–11 minutes."
      compact={compact}
    />
  );
}

function Failed({
  error,
  busy,
  onRetry,
}: {
  error: string | null;
  busy: boolean;
  onRetry: () => void;
}) {
  return (
    <>
      <p className="text-destructive">{error || "Chaptering failed."}</p>
      <Button size="xs" variant="outline" disabled={busy} onClick={onRetry}>
        Try again
      </Button>
    </>
  );
}
