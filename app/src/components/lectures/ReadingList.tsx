import { memo, useCallback } from "react";

import { cn } from "@/lib/utils";
import { useTranscriptSearch } from "@/hooks/useTranscriptSearch";
import type { ReadingLine } from "@/lib/db";
import { InlineMd } from "@/components/markdown/MdComponents";
import { FollowList, Highlight, SearchField } from "@/components/media/FollowList";
import {
  TranscriptModePicker,
  type TranscriptModePickerProps,
} from "@/components/lectures/TranscriptModePicker";
import { RegenerateRow, RunStatus } from "@/components/lectures/RunStatus";
import { PanelEmpty } from "@/components/media/MediaDock";
import { READING_PHASE_LABEL, type ReadingRunProgress } from "@/lib/lectures";
import { fmtClockSecs } from "@/lib/media";
import type { ReadingStatus } from "@/hooks/useLectureReading";

export interface ReadingListProps {
  lines: ReadingLine[];
  /** Which line the playhead is in; -1 before the first line. */
  activeLineIdx: number;

  status: ReadingStatus;
  /** `reading_error`, shown verbatim in the footer. With no lines the picker
   *  shows it and this list is not mounted. */
  error: string | null;
  /** When this session claimed the run, for the elapsed clock. */
  since: number | null;
  /** What the run is doing right now, or null before it has said. */
  progress: ReadingRunProgress | null;
  /** A run is in flight (Rust refuses a second one). */
  busy: boolean;
  onWrite: (force: boolean) => void;

  /** The job watches the recording, so it needs the file on disk. */
  downloaded: boolean;

  /** The register picker, drawn in this list's search row. */
  picker: TranscriptModePickerProps;

  /** The dock is open — for `FollowList`, which stays mounted and slides. */
  open: boolean;
  /** The Transcript tab is in front — drives `FollowList`'s reopen-scroll. */
  active: boolean;
  /** Tracking playback; shared with the verbatim list (only one is mounted). */
  following: boolean;
  onScrollAway: () => void;
  onBackToLive: () => void;

  onSeek: (seconds: number) => void;
}

/**
 * The Transcript tab's Enhanced register: the same recording after the reading
 * copy has been over it (`docs/chapters.md`) — filler dropped, notation fixed
 * off the slide, spoken maths set as `$…$`. One line spans several cues by
 * design, so the highlight moves in bigger steps than the verbatim list.
 * `FollowList` carries the virtualizer, follow-scroll and search for both.
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
  const { query, setQuery, needle, searching, rows, followIdx } =
    useTranscriptSearch(lines, activeLineIdx);
  // Playback moves the highlight without invalidating every line measurement.
  const getLineKey = useCallback((row: number) => rows[row], [rows]);

  const running = status === "running";

  // With no lines this is only mounted for a run that hasn't committed its
  // first window; otherwise the picker offers the job (see `TranscriptPanel`).
  if (lines.length === 0) {
    return (
      <>
        <div className="flex shrink-0 items-center justify-end px-1.5 pt-1.5">
          <TranscriptModePicker {...picker} />
        </div>
        <div className="flex-1 min-h-0 overflow-y-auto">
          <PanelEmpty>
            <ReadingRun since={since} progress={progress} />
          </PanelEmpty>
        </div>
      </>
    );
  }

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
        // Keyed by line, not row, so a measured height survives a query.
        getItemKey={getLineKey}
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
          // A paragraph opens at each slide change. Padding, not margin: the
          // virtualizer measures the border box, so a margin would overlap
          // the next row. Off while searching, where rows aren't consecutive.
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
              {/* Same classes as the verbatim list's cue button. */}
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
                  {fmtClockSecs(line.start_seconds)}
                </span>
                <span className="min-w-0 flex-1 text-[11.5px] leading-relaxed">
                  {/* `InlineMd` because a `<p>` in a `<button>` closes it early
                      in WebKit; it can't carry a `<mark>`, so a search hit
                      shows plain text. */}
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

      {/* Lines already committed stay on screen through a failed or running
          regenerate, so both report here rather than in their place. */}
      <div className="shrink-0 border-t border-border px-2 py-1.5">
        {running ? (
          <ReadingRun since={since} progress={progress} compact />
        ) : (
          <RegenerateRow
            disabled={busy || !downloaded}
            title={downloaded ? undefined : "Download the recording first"}
            error={status === "error" ? error : null}
            onClick={() => onWrite(true)}
          />
        )}
      </div>
    </>
  );
});

function ReadingRun({
  since,
  progress,
  compact,
}: {
  since: number | null;
  progress: ReadingRunProgress | null;
  compact?: boolean;
}) {
  // Unlike chaptering, this is a countable run of agent turns, so "3/7" is a fact.
  const window = progress?.window ?? null;
  return (
    <RunStatus
      since={since}
      label={progress ? READING_PHASE_LABEL[progress.phase] : "Enhancing the transcript"}
      progress={progress}
      counter={
        window && window.total > 0
          ? `${Math.min(window.done + 1, window.total)}/${window.total}`
          : undefined
      }
      note="One turn per ten minutes of recording."
      compact={compact}
    />
  );
}
