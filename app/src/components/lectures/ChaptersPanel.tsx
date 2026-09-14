import { memo, useEffect, useRef, useState } from "react";
import { CircleNotch } from "@phosphor-icons/react";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import type { Chapter } from "@/lib/db";
import { chapterEnds, fmtTime } from "@/lib/lectures";
import type { ChapterStatus } from "@/hooks/useLectureChapters";

/**
 * A chapter's length, rounded to the minute.
 *
 * The job will not name anything under about three minutes a chapter
 * (docs/chapters.md), so the seconds are noise — and the dock is 200px wide at
 * its narrowest, where `12m 04s` next to a wrapping title is what pushes the
 * title onto a third line. Body font with `tabular-nums`, like every other
 * number in the app.
 */
function fmtSpan(seconds: number): string {
  const m = Math.round(seconds / 60);
  return m < 1 ? "<1m" : `${m}m`;
}

export interface ChaptersPanelProps {
  chapters: Chapter[];
  /** Which chapter the playhead is in; -1 before the first one starts. */
  activeIdx: number;
  /** The player's duration — the element's where one is loaded, since the
   *  last chapter ends there and the catalogue's figure runs short. */
  duration: number;
  status: ChapterStatus;
  /** `chapter_error`, shown verbatim: it is the agent's own failure. */
  error: string | null;
  /** When this session claimed the run, for the elapsed clock. */
  since: number | null;
  /** A run is in flight — the only thing standing between two turns being
   *  spent on one lecture, since Rust refuses the second call outright. */
  busy: boolean;
  /** Chaptering watches the recording, so it needs the file on disk. */
  downloaded: boolean;
  onSeek: (seconds: number) => void;
  onFind: (force: boolean) => void;
}

/**
 * The chapter list — a lecture read as the five to twelve things it is about.
 *
 * **Not the transcript's follow machinery.** That code is wound around a
 * virtualizer and earns its two-stage handover and countdown ring on ~2500
 * rows; twelve fit the panel with room over. So the current card is brought
 * into view with `scrollIntoView({ block: "nearest" })` and nothing else —
 * no pill, no window, no ring.
 *
 * Every state here is a real one: a lecture that is not downloaded, one that
 * has never been chaptered, a run in flight, a run that failed, and the list.
 * Nothing is hand-editable — chapters are derived data an agent writes, so the
 * only two affordances are Find chapters and Regenerate.
 */
export const ChaptersPanel = memo(function ChaptersPanel({
  chapters,
  activeIdx,
  duration,
  status,
  error,
  since,
  busy,
  downloaded,
  onSeek,
  onFind,
}: ChaptersPanelProps) {
  const activeRef = useRef<HTMLButtonElement | null>(null);

  // The list follows playback: the summary beside the video is the one being
  // talked about. `nearest` scrolls the least that will do, so a card already
  // on screen does not jump to the middle of the panel under the eye.
  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: "nearest" });
  }, [activeIdx]);

  if (chapters.length === 0) {
    return (
      <div className="flex-1 min-h-0 overflow-y-auto">
        {!downloaded ? (
          <Empty>
            <p>Chapters are found by watching the recording.</p>
            <p className="text-muted-foreground">Download it first.</p>
          </Empty>
        ) : status === "running" ? (
          <Empty>
            <Running since={since} />
          </Empty>
        ) : status === "error" ? (
          <Empty>
            <Failed error={error} busy={busy} onRetry={() => onFind(true)} />
          </Empty>
        ) : (
          <Empty>
            <Button size="xs" disabled={busy} onClick={() => onFind(false)}>
              Find chapters
            </Button>
            <p className="text-muted-foreground">Takes 8–11 minutes.</p>
          </Empty>
        )}
      </div>
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
                "mb-0.5 flex w-full items-start gap-2 rounded px-2 py-1.5 text-left transition-colors",
                active
                  ? "bg-brand/12"
                  : "text-muted-foreground hover:bg-surface hover:text-foreground",
              )}
            >
              <span
                className={cn(
                  "w-10 shrink-0 pt-px text-[10px] tabular-nums",
                  active ? "text-brand/70" : "opacity-60",
                )}
              >
                {fmtTime(c.start_seconds)}
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
                {active && (
                  <span className="mt-1 block text-[11px] leading-relaxed text-muted-foreground">
                    {c.summary}
                  </span>
                )}
              </span>
            </button>
          );
        })}
      </div>

      {/* A regenerate that fails leaves the chapters that were there, so the
          failure belongs beside them rather than in place of them. */}
      <div className="shrink-0 border-t border-border px-2 py-1.5">
        {status === "running" ? (
          <Running since={since} compact />
        ) : (
          <div className="flex items-center gap-2">
            <Button
              size="xs"
              variant="ghost"
              disabled={busy}
              onClick={() => onFind(true)}
              className="-ml-1 text-muted-foreground"
            >
              Regenerate
            </Button>
            {status === "error" && error && (
              <span className="min-w-0 flex-1 truncate text-[10px] text-destructive" title={error}>
                {error}
              </span>
            )}
          </div>
        )}
      </div>
    </>
  );
});

function Empty({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-1.5 px-4 py-6 text-center text-[11px] leading-relaxed">
      {children}
    </div>
  );
}

/**
 * Eight to eleven minutes of ffmpeg and one very long agent turn, with no
 * progress to report in between — so the clock is the whole message. It counts
 * up rather than filling a bar towards an estimate, because a bar that reaches
 * the end and keeps waiting is exactly what "hung" looks like.
 *
 * In-flight is `brand`, the accent the app spends on work in progress.
 */
function Running({ since, compact }: { since: number | null; compact?: boolean }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (since === null) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [since]);

  // A run already in flight when the app started has no start time anywhere:
  // `chaptered_at` is stamped by a terminal status only.
  const elapsed = since === null ? null : Math.max(0, Math.floor((now - since) / 1000));

  return (
    <div
      className={cn(
        "flex items-center gap-2 text-[11px] text-brand",
        !compact && "flex-col gap-1.5",
      )}
    >
      <span className="flex items-center gap-1.5">
        <CircleNotch size={12} className="animate-spin" />
        Finding chapters
        {elapsed !== null && (
          <span className="tabular-nums text-muted-foreground">{fmtTime(elapsed)}</span>
        )}
      </span>
      <span className="text-muted-foreground">Usually 8–11 minutes.</span>
    </div>
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
