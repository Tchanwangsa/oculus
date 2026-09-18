import { memo, useEffect, useMemo, useState, type ReactNode } from "react";
import { CircleNotch } from "@phosphor-icons/react";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import type { ReadingLine } from "@/lib/db";
import { InlineMd } from "@/components/markdown/MdComponents";
import { FollowList, Highlight, SearchField } from "@/components/lectures/FollowList";
import {
  TranscriptModePicker,
  type TranscriptModePickerProps,
} from "@/components/lectures/TranscriptModePicker";
import { READING_PHASE_LABEL, fmtTime, type ReadingRunProgress } from "@/lib/lectures";
import { toolVerb } from "@/lib/harness";
import type { ReadingStatus } from "@/hooks/useLectureReading";

export interface ReadingListProps {
  lines: ReadingLine[];
  /** Which line the playhead is in — `spanAt` over the lines' starts, the
   *  same arithmetic the chapter list does; -1 before the first line. */
  activeLineIdx: number;

  status: ReadingStatus;
  /** `reading_error`, shown as the agent left it. With lines on screen it is
   *  a line in the footer beside them; with none, it is the picker's own row
   *  that carries it, and this list is not mounted at all. */
  error: string | null;
  /** When this session claimed the run, for the elapsed clock. */
  since: number | null;
  /** What the run is doing right now, or null before it has said. */
  progress: ReadingRunProgress | null;
  /** A run is in flight — the only thing standing between two turns being
   *  spent on one lecture, since Rust refuses the second call outright. */
  busy: boolean;
  onWrite: (force: boolean) => void;

  /** The job watches the recording as well as reading the transcript, so it
   *  needs the file on disk. The transcript it is a rewrite of is already
   *  proven: this list only renders inside a tab the dock drops when there
   *  are no cues. */
  downloaded: boolean;

  /** The register picker, which sits in this list's search row. It is the
   *  job's control as well as the switch, so the row it needs is the same one
   *  the search field is in — see `TranscriptModePicker`. */
  picker: TranscriptModePickerProps;

  /** The dock is open — for `FollowList`, which stays mounted and slides. */
  open: boolean;
  /** The Transcript tab is in front — drives `FollowList`'s reopen-scroll. */
  active: boolean;
  /** The list is tracking playback rather than being read by hand. Shared
   *  with the verbatim list: only one of the two is mounted at a time. */
  following: boolean;
  onScrollAway: () => void;
  onBackToLive: () => void;

  onSeek: (seconds: number) => void;
}

/**
 * The Transcript tab in its Enhanced register: the recording's own words,
 * read back as text.
 *
 * The verbatim list one toggle over is every three-second cue exactly as the
 * recogniser heard it. This is the same span of recording after the reading
 * copy has been over it (`docs/chapters.md`): the filler dropped, the
 * notation fixed off the slide, and the spoken maths — "a naught ket zero" —
 * set as `$a_0|0\\rangle$`.
 *
 * **It is deliberately coarser than the cue list, and that is not a
 * regression.** A cue is a fragment and maths spans several of them, so one
 * line covers two to six cues by design; the job's validator is what holds
 * that ratio. So there are fewer, longer rows here and the highlight moves in
 * bigger steps — ten to twenty seconds rather than three. That is the
 * granularity of reading rather than of following along, which is what the
 * other register is for.
 *
 * It is the transcript's own list — `FollowList` carries the virtualizer, the
 * follow-scroll, the Back-to-live pill and the search for both registers, so
 * switching between them keeps the machinery and changes only the rows.
 *
 * Nothing is hand-editable: the lines are derived data an agent writes, like
 * `pages` and `parse_status`, so the only affordance is the job itself —
 * offered as a button from the empty state and as Regenerate in the footer
 * once there are lines.
 */
export const ReadingList = memo(function ReadingList({
  lines,
  activeLineIdx,
  status,
  error,
  since,
  progress,
  busy,
  onWrite,
  downloaded,
  open,
  active,
  following,
  onScrollAway,
  onBackToLive,
  onSeek,
  picker,
}: ReadingListProps) {
  // ── Search ───────────────────────────────────────────────────────────────

  // The verbatim list's bridge, over a fiftieth of the rows: the virtualizer
  // is told about rows, playback is about lines, and `rows[i]` is the only way
  // from one to the other.
  const [query, setQuery] = useState("");
  const needle = query.trim().toLowerCase();
  const searching = needle.length > 0;

  const rows = useMemo(() => {
    if (!needle) return lines.map((_, i) => i);
    const out: number[] = [];
    for (let i = 0; i < lines.length; i++) {
      if (lines[i].text.toLowerCase().includes(needle)) out.push(i);
    }
    return out;
  }, [lines, needle]);

  // Suspended while searching, for the verbatim list's reason: the playing row
  // may not be in a list the query is choosing the contents of.
  const followIdx = searching ? -1 : activeLineIdx;

  const running = status === "running";

  // ── Before the first window lands ────────────────────────────────────────

  // The only way to be here with nothing to show is a run that has claimed
  // the lecture and not committed its first window yet. With no lines and no
  // run, `modeInFront` keeps the standard list in front and the picker is
  // what offers the job (see `TranscriptPanel`) — so there is no "nothing
  // yet" state here and no second Write button. One control, in one place.
  if (lines.length === 0) {
    return (
      <>
        <div className="flex shrink-0 items-center justify-end px-1.5 pt-1.5">
          <TranscriptModePicker {...picker} />
        </div>
        <div className="flex-1 min-h-0 overflow-y-auto">
          <Empty>
            <Running since={since} progress={progress} />
          </Empty>
        </div>
      </>
    );
  }

  // ── The list ─────────────────────────────────────────────────────────────

  return (
    <>
      <div className="flex shrink-0 items-center gap-1.5 px-1.5 pt-1.5">
        <SearchField
          value={query}
          onChange={setQuery}
          placeholder="Search"
          count={searching ? rows.length : undefined}
          className="min-w-0 flex-1 shrink p-0"
        />
        <TranscriptModePicker {...picker} />
      </div>
      <FollowList
        count={rows.length}
        followIdx={followIdx}
        // Keyed by *line* index, not row: a row's measured height then
        // survives the query that moved it to a different row. Lines wrap to
        // three or four of them, so those measurements are worth keeping.
        getItemKey={(i) => rows[i]}
        open={open}
        active={active}
        following={following}
        onScrollAway={onScrollAway}
        onBackToLive={onBackToLive}
        resetKey={needle}
        overlay={searching && rows.length === 0 ? "No matches" : undefined}
        renderRow={(row, item, measure) => {
          const lineIdx = rows[row];
          const line = lines[lineIdx];
          const isActive = lineIdx === activeLineIdx;
          // A paragraph opens at each slide change (`para`, set in Rust off
          // the detected boundaries, never asked of the model). The gap is
          // padding on a positioned wrapper rather than a margin on the
          // button: the virtualizer measures a row's border box, and a margin
          // on an absolutely positioned row would shift it down by an amount
          // the list never learns of — onto the row below. Suppressed while
          // searching, where consecutive rows are not consecutive lines and a
          // paragraph is not a thing the results have.
          const para = !searching && line.para === 1 && row > 0;
          return (
            <div
              data-index={item.index}
              ref={measure}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                transform: `translateY(${item.start}px)`,
              }}
              className={cn(para && "pt-2")}
            >
              {/* The verbatim list's cue button, to the class: the two
                  registers are one recording read two ways, and a line should
                  sit where its cues would. */}
              <button
                onClick={() => onSeek(line.start_seconds)}
                className={cn(
                  "w-full text-left text-[11px] px-2 py-1 rounded flex gap-2 items-start",
                  isActive
                    ? "bg-brand/12 text-brand"
                    : "text-muted-foreground hover:text-foreground hover:bg-surface",
                )}
              >
                <span className="tabular-nums text-[10px] shrink-0 pt-px w-10 opacity-60">
                  {fmtTime(line.start_seconds)}
                </span>
                <span className="min-w-0 flex-1 text-[11.5px] leading-relaxed">
                  {/* The model writes `$…$` maths, and `InlineMd` is the
                      renderer that is legal inside a button — a `<p>` in a
                      `<button>` closes the button early in WebKit. It cannot
                      take a `<mark>` through it, though, so while searching
                      the line is the plain text with the needle marked, and a
                      hit's maths shows as its source until the query is
                      cleared. */}
                  {searching ? (
                    <Highlight text={line.text} needle={needle} />
                  ) : (
                    <InlineMd text={line.text} />
                  )}
                </span>
              </button>
            </div>
          );
        }}
      />

      {/* A regenerate that fails leaves the lines that were there — the job
          commits window by window and keeps the windows before the failure —
          so the message belongs beside them rather than in place of them. The
          same goes for a run in flight: its lines are already arriving. */}
      <div className="shrink-0 border-t border-border px-2 py-1.5">
        {running ? (
          <Running since={since} progress={progress} compact />
        ) : (
          <div className="flex items-center gap-2">
            <Button
              size="xs"
              variant="ghost"
              disabled={busy || !downloaded}
              title={downloaded ? undefined : "Download the recording first"}
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

function Empty({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-1.5 px-4 py-6 text-center text-[11px] leading-relaxed">
      {children}
    </div>
  );
}

/**
 * The step under the phase: what the job is touching right this second.
 *
 * A tool call is the agent's own account of the minutes and is said in the
 * timeline's words (`toolVerb`) rather than a second vocabulary. The
 * countable phases say how far through they are — a real fraction of a decode
 * that is happening, not an estimate of a turn that has not answered yet.
 */
function stepDetail(p: ReadingRunProgress): string | null {
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
 * Minutes of ffmpeg and one long agent turn per window, reported as the step
 * it is on: the phase in `brand`, and under it the file being read or the
 * second being decoded.
 *
 * **Still no bar.** The clock counts up and there is no fill towards an
 * estimate, because a bar that reaches the end and keeps waiting is exactly
 * what hung looks like. The *steps* are real, though: they come off the same
 * event stream the chat timeline draws, and a job that has gone quiet looks
 * different from one that is working.
 *
 * **This one can say how far through itself it is**, unlike chaptering: it is
 * a countable sequence of agent turns, so "3/7" is a fact rather than an
 * estimate of a single turn that has not answered.
 *
 * A run already in flight at app launch has neither a start time nor a step
 * until its next one — nothing about it is persisted, and `reading_written_at`
 * is stamped by a terminal status only — so both are optional and the spinner
 * alone is a valid state.
 */
function Running({
  since,
  progress,
  compact,
}: {
  since: number | null;
  progress: ReadingRunProgress | null;
  compact?: boolean;
}) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (since === null) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [since]);

  const elapsed = since === null ? null : Math.max(0, Math.floor((now - since) / 1000));
  const label = progress ? READING_PHASE_LABEL[progress.phase] : "Enhancing the transcript";
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
        <span className="truncate">{label}</span>
        {window && window.total > 0 && (
          <span className="shrink-0 tabular-nums text-muted-foreground">
            {Math.min(window.done + 1, window.total)}/{window.total}
          </span>
        )}
        {elapsed !== null && (
          <span className="shrink-0 tabular-nums text-muted-foreground">{fmtTime(elapsed)}</span>
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
      {!compact && (
        <span className="text-muted-foreground">One turn per ten minutes of recording.</span>
      )}
    </div>
  );
}
