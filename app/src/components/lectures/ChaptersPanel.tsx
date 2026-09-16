import { memo, useEffect, useRef, useState } from "react";
import { CircleNotch } from "@phosphor-icons/react";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import type { Chapter } from "@/lib/db";
import { InlineMd } from "@/components/markdown/MdComponents";
import {
  CHAPTER_PHASE_LABEL,
  chapterEnds,
  fmtTime,
  type ChapterRunProgress,
} from "@/lib/lectures";
import { toolVerb } from "@/lib/harness";
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
  /** What the run is doing right now, or null before it has said. */
  progress: ChapterRunProgress | null;
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
  progress,
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
            <Running since={since} progress={progress} />
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
                {/* Through the markdown renderer, because the agent writes
                    these with formulas in them — `$\log_2 N$` sat in the
                    panel as its own source until it did. Inline-only: this is
                    inside the button that seeks, and a `<p>` in a `<button>`
                    closes the button early in WebKit. */}
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

      {/* A regenerate that fails leaves the chapters that were there, so the
          failure belongs beside them rather than in place of them. */}
      <div className="shrink-0 border-t border-border px-2 py-1.5">
        {status === "running" ? (
          <Running since={since} progress={progress} compact />
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
 * The step under the phase: what the job is touching right this second.
 *
 * A tool call is the agent's own account of the nine minutes and is said in
 * the timeline's words (`toolVerb`) rather than a second vocabulary. The two
 * countable phases say how far through they are — a real fraction of a decode
 * that is happening, not an estimate of a turn that has not answered yet.
 */
function stepDetail(p: ChapterRunProgress): string | null {
  const title = p.detail?.trim();
  if (title) return p.kind ? `${toolVerb(p.kind, false)} ${title}` : title;
  if (p.kind) return toolVerb(p.kind, false);
  if (p.done !== null && p.total) {
    // Clamped because the two numbers come from different places: the decode
    // counts frames off the file, the total is the catalogue's duration, and
    // Echo360's figure runs a few seconds short of the recording.
    const done = Math.min(p.done, p.total);
    return p.phase === "decoding"
      ? `${fmtTime(done)} of ${fmtTime(p.total)}`
      : `${done} of ${p.total}`;
  }
  return null;
}

/**
 * Eight to eleven minutes of ffmpeg and one very long agent turn, reported as
 * the step it is on: the phase in `brand`, and under it the file being read or
 * the second being decoded.
 *
 * **Still no bar.** The clock counts up and there is no fill towards an
 * estimate, because a bar that reaches the end and keeps waiting is exactly
 * what hung looks like — the turn is most of the run and nothing can say how
 * far through it is. What changed is that the *steps* are real: they come off
 * the same event stream the chat timeline draws, and a job that has gone quiet
 * now looks different from one that is working.
 *
 * A run already in flight at app launch has neither a start time nor a step
 * until its next one — nothing about it is persisted — so both are optional
 * and the spinner alone is a valid state.
 */
function Running({
  since,
  progress,
  compact,
}: {
  since: number | null;
  progress: ChapterRunProgress | null;
  compact?: boolean;
}) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (since === null) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [since]);

  // A run already in flight when the app started has no start time anywhere:
  // `chaptered_at` is stamped by a terminal status only.
  const elapsed = since === null ? null : Math.max(0, Math.floor((now - since) / 1000));
  const phase = progress ? CHAPTER_PHASE_LABEL[progress.phase] : "Finding chapters";
  const detail = progress ? stepDetail(progress) : null;

  return (
    <div
      className={cn(
        "flex w-full min-w-0 flex-col gap-0.5 text-[11px] text-brand",
        !compact && "items-center text-center",
      )}
    >
      <span className="flex max-w-full items-center gap-1.5">
        <CircleNotch size={12} className="shrink-0 animate-spin" />
        <span className="truncate">{phase}</span>
        {elapsed !== null && (
          <span className="shrink-0 tabular-nums text-muted-foreground">
            {fmtTime(elapsed)}
          </span>
        )}
      </span>
      {/* The agent's file names run long and the dock is 200px at its
          narrowest, so the line truncates and keeps the whole of it in the
          tooltip. */}
      {detail && (
        <span className="block max-w-full truncate text-muted-foreground" title={detail}>
          {detail}
        </span>
      )}
      {!compact && <span className="text-muted-foreground">Usually 8–11 minutes.</span>}
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
