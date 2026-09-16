import { memo, useEffect, useRef, useState } from "react";
import { CircleNotch } from "@phosphor-icons/react";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { CompactMd } from "@/components/markdown/MdComponents";
import type { RecapNote } from "@/lib/db";
import { RECAP_PHASE_LABEL, fmtTime, type RecapRunProgress } from "@/lib/lectures";
import { toolVerb } from "@/lib/harness";
import type { RecapStatus } from "@/hooks/useLectureRecap";

export interface RecapPanelProps {
  notes: RecapNote[];
  /** Which note the playhead is in; -1 before the first one starts. */
  activeIdx: number;
  status: RecapStatus;
  /** `recap_error`, shown verbatim: it is the agent's own failure. */
  error: string | null;
  /** When this session claimed the run, for the elapsed clock. */
  since: number | null;
  progress: RecapRunProgress | null;
  busy: boolean;
  /** The recap watches the recording, so it needs the file on disk. */
  downloaded: boolean;
  /** …and reads the whole transcript, unlike chaptering, where it is optional
   *  reading. Rust refuses the run without one, so the panel says so first. */
  hasTranscript: boolean;
  onSeek: (seconds: number) => void;
  onWrite: (force: boolean) => void;
}

/**
 * The recap — a lecture read as the twenty or forty moments it passed through,
 * each with what was on the slide and what was said over it.
 *
 * **Every note's body is on screen, where a chapter's summary is only on the
 * one being played.** The two tabs answer different questions and so are read
 * differently: chapters are navigation, and a list of twelve titles is the
 * whole point of them, while a recap is the lecture written down — something
 * to read through after the fact, not a place to click. Following playback
 * still happens, as a highlight rather than as the only way to see any prose.
 *
 * **Notes appear while the job is still running.** A recap commits one window
 * at a time (docs/chapters.md), so the list and the progress line are shown
 * together rather than as alternatives — which is the visible half of a design
 * decision that would otherwise only exist in the database.
 */
export const RecapPanel = memo(function RecapPanel({
  notes,
  activeIdx,
  status,
  error,
  since,
  progress,
  busy,
  downloaded,
  hasTranscript,
  onSeek,
  onWrite,
}: RecapPanelProps) {
  const activeRef = useRef<HTMLDivElement | null>(null);

  // `nearest` scrolls the least that will do, so a note already on screen does
  // not jump under the eye. The ref is on the whole note rather than on its
  // heading, which is what makes `nearest` useful here: a note that fits is
  // brought fully into view, and one taller than the panel is aligned to its
  // top — where a heading-only target would scroll the prose back off.
  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: "nearest" });
  }, [activeIdx]);

  if (notes.length === 0) {
    return (
      <div className="flex-1 min-h-0 overflow-y-auto">
        {!downloaded ? (
          <Empty>
            <p>A recap is written from the recording.</p>
            <p className="text-muted-foreground">Download it first.</p>
          </Empty>
        ) : !hasTranscript ? (
          <Empty>
            <p>A recap reads the whole transcript.</p>
            <p className="text-muted-foreground">This lecture has none on disk.</p>
          </Empty>
        ) : status === "running" ? (
          <Empty>
            <Running since={since} progress={progress} />
          </Empty>
        ) : status === "error" ? (
          <Empty>
            <p className="text-destructive">{error || "The recap failed."}</p>
            <Button size="xs" variant="outline" disabled={busy} onClick={() => onWrite(true)}>
              Try again
            </Button>
          </Empty>
        ) : (
          <Empty>
            <Button size="xs" disabled={busy} onClick={() => onWrite(false)}>
              Write recap
            </Button>
            <p className="text-muted-foreground">
              One agent turn per ten minutes of recording.
            </p>
          </Empty>
        )}
      </div>
    );
  }

  return (
    <>
      <div className="flex-1 min-h-0 overflow-y-auto px-1.5 py-2">
        {notes.map((note, i) => {
          const active = i === activeIdx;
          return (
            <div
              key={note.idx}
              ref={active ? activeRef : undefined}
              className={cn(
                "mb-1 rounded px-2 py-1.5 transition-colors",
                active && "bg-brand/12",
              )}
            >
              {/* Only the heading seeks. The body is markdown — paragraphs,
                  the occasional list, a formula — and none of that may sit
                  inside a button, so the click target is the line above it
                  rather than the whole card the Chapters tab uses. */}
              <button
                type="button"
                onClick={() => onSeek(note.start_seconds)}
                className="flex w-full items-baseline gap-2 text-left"
              >
                <span
                  className={cn(
                    "w-10 shrink-0 text-[10px] tabular-nums",
                    active ? "text-brand/70" : "opacity-60",
                  )}
                >
                  {fmtTime(note.start_seconds)}
                </span>
                <span
                  className={cn(
                    "min-w-0 flex-1 text-[11px] font-medium leading-snug",
                    active ? "text-brand" : "text-foreground/90",
                  )}
                >
                  {note.label || "Note"}
                </span>
              </button>
              {/* Full width under the heading rather than indented to line
                  up with the label: a 40px timestamp gutter leaves 150px for
                  the prose in a 220px dock, which wraps it to two words a
                  line. The heading's own gutter is what keeps the two legible
                  as a note. */}
              <CompactMd
                text={note.body}
                className="mt-0.5 text-[11px] leading-relaxed text-muted-foreground"
              />
            </div>
          );
        })}
      </div>

      {/* The same footer the Chapters tab has, and it carries the run: a recap
          in flight already has notes on screen, so its progress belongs beside
          them rather than in place of them. */}
      <div className="shrink-0 border-t border-border px-2 py-1.5">
        {status === "running" ? (
          <Running since={since} progress={progress} compact />
        ) : (
          <div className="flex items-center gap-2">
            <Button
              size="xs"
              variant="ghost"
              disabled={busy}
              onClick={() => onWrite(true)}
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
 * The step under the phase, in the timeline's words (`toolVerb`) rather than a
 * second vocabulary for the same enum.
 */
function stepDetail(p: RecapRunProgress): string | null {
  const title = p.detail?.trim();
  if (title) return p.kind ? `${toolVerb(p.kind, false)} ${title}` : title;
  if (p.kind) return toolVerb(p.kind, false);
  if (p.done !== null && p.total) {
    // Clamped: the decode counts frames off the file while the total is the
    // catalogue's duration, and Echo360's figure runs a few seconds short.
    const done = Math.min(p.done, p.total);
    return p.phase === "decoding"
      ? `${fmtTime(done)} of ${fmtTime(p.total)}`
      : `${done} of ${p.total}`;
  }
  return null;
}

/**
 * **This job can say how far through itself it is, and the chaptering one
 * cannot.** A recap is a countable sequence of agent turns, so "3 of 7" is a
 * fact rather than an estimate of a single turn that has not answered yet —
 * which is why the window counter is here and there is still no bar next to
 * the chapter spinner.
 */
function Running({
  since,
  progress,
  compact,
}: {
  since: number | null;
  progress: RecapRunProgress | null;
  compact?: boolean;
}) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (since === null) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [since]);

  // A run already in flight when the app started has no start time anywhere:
  // `recapped_at` is stamped by a terminal status only.
  const elapsed = since === null ? null : Math.max(0, Math.floor((now - since) / 1000));
  const phase = progress ? RECAP_PHASE_LABEL[progress.phase] : "Writing a recap";
  const detail = progress ? stepDetail(progress) : null;
  const window = progress?.window ?? null;

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
        {window && window.total > 0 && (
          <span className="shrink-0 tabular-nums text-muted-foreground">
            {Math.min(window.done + 1, window.total)}/{window.total}
          </span>
        )}
        {elapsed !== null && (
          <span className="shrink-0 tabular-nums text-muted-foreground">
            {fmtTime(elapsed)}
          </span>
        )}
      </span>
      {/* The agent's file names run long and the dock is 220px at its
          narrowest, so the line truncates and keeps the whole of it in the
          tooltip. */}
      {detail && (
        <span className="block max-w-full truncate text-muted-foreground" title={detail}>
          {detail}
        </span>
      )}
    </div>
  );
}
