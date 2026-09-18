import { useState, type ReactNode } from "react";
import { ArrowClockwise, CaretDown, Check, CircleNotch, Sparkle } from "@phosphor-icons/react";

import { cn } from "@/lib/utils";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { READING_PHASE_LABEL, type ReadingRunProgress } from "@/lib/lectures";
import type { ReadingStatus } from "@/hooks/useLectureReading";
import type { TranscriptMode } from "@/stores/playerPrefsStore";

export const MODE_LABEL: Record<TranscriptMode, string> = {
  standard: "Standard",
  enhanced: "Enhanced",
};

const MODE_HINT: Record<TranscriptMode, string> = {
  standard: "Every cue, as recorded",
  enhanced: "Rewritten, with the maths set",
};

export interface TranscriptModePickerProps {
  value: TranscriptMode;
  onChange: (mode: TranscriptMode) => void;

  /** The reading job's state — the Enhanced row is the job's own control. */
  status: ReadingStatus;
  /** `reading_error`, shown verbatim under the row: it is the agent's own
   *  failure, and this is the only place a failed run with nothing to show
   *  for it can surface. */
  error: string | null;
  progress: ReadingRunProgress | null;
  /** A run is in flight — Rust refuses the second call outright. */
  busy: boolean;
  /** The job watches the recording as well as reading the transcript. */
  downloaded: boolean;
  /** There is something to switch *to*. Until there is, picking Enhanced is
   *  asking for it to be written. */
  hasLines: boolean;
  onEnhance: (force: boolean) => void;
}

/**
 * Which register the Transcript tab is in — and, until there is one to switch
 * to, how the enhanced copy gets written in the first place.
 *
 * **It is the source switcher's shape** (`SourceControls.tsx`): a row per
 * option, each with what it is under its name, and the row for the thing that
 * is not on disk yet says *why* and starts the job that fetches it. A capture
 * lists the camera it has not downloaded because that panel is where you find
 * out the camera exists; the same is true here, and a Write button parked in
 * an empty panel was the alternative — a control you only meet by first
 * choosing the empty view.
 *
 * It replaced a segmented Verbatim/Enhanced toggle, which cost ~100px of a
 * 220px header to say two words and had nowhere to put the job.
 *
 * Themed surfaces and not the control bar's glass, because this sits in the
 * dock rather than over the frame.
 */
export function TranscriptModePicker({
  value,
  onChange,
  status,
  error,
  progress,
  busy,
  downloaded,
  hasLines,
  onEnhance,
}: TranscriptModePickerProps) {
  // Open in state rather than left to Radix, because picking a register is the
  // end of the errand either way: switching closes, and so does asking for the
  // copy to be written — the enhanced view *is* the progress, since the job
  // commits window by window and its lines arrive into the list behind this.
  const [open, setOpen] = useState(false);

  const running = status === "running";
  const failed = status === "error" && !hasLines;
  /** Picking Enhanced asks for a run rather than switching. */
  const wouldWrite = !hasLines && !running && downloaded;

  const pick = (mode: TranscriptMode) => {
    if (mode === "enhanced" && !downloaded) return;
    if (mode === "enhanced" && wouldWrite) onEnhance(status === "error");
    onChange(mode);
    setOpen(false);
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        className={cn(
          "flex h-6 shrink-0 items-center gap-1 rounded-full bg-surface px-2 text-[10px]",
          "text-muted-foreground transition-colors hover:text-foreground",
          "data-[state=open]:text-foreground",
        )}
        aria-label={`Showing the ${MODE_LABEL[value].toLowerCase()} transcript — change`}
      >
        {/* A run started from here keeps saying so while you read the other
            register: it is minutes long and the only other sign of it is the
            list it has not filled yet. */}
        {running && <CircleNotch size={9} className="shrink-0 animate-spin text-brand" />}
        {MODE_LABEL[value]}
        <CaretDown size={8} weight="bold" className="opacity-60" />
      </PopoverTrigger>

      <PopoverContent align="end" className="w-56 p-1">
        <Row
          label={MODE_LABEL.standard}
          hint={MODE_HINT.standard}
          onClick={() => pick("standard")}
          active={value === "standard"}
          status={value === "standard" ? <Check size={13} weight="bold" /> : null}
        />
        <Row
          label={MODE_LABEL.enhanced}
          // The row carries the reason it cannot be picked, and the agent's
          // own message when a run has failed with nothing to show for it —
          // there is no panel behind it to put that in.
          hint={
            !downloaded
              ? "Download the recording first"
              : failed
                ? error || "The last run failed"
                : running
                  ? progress
                    ? READING_PHASE_LABEL[progress.phase]
                    : "Writing…"
                  : MODE_HINT.enhanced
          }
          hintClass={failed ? "text-destructive" : running ? "text-brand" : undefined}
          disabled={!downloaded || (running && !hasLines) || (busy && !hasLines)}
          onClick={() => pick("enhanced")}
          active={value === "enhanced"}
          status={
            running ? (
              <span className="flex items-center gap-1 text-[10px] tabular-nums text-brand">
                <CircleNotch size={11} className="animate-spin" />
                {progress?.window && progress.window.total > 0
                  ? `${Math.min(progress.window.done + 1, progress.window.total)}/${progress.window.total}`
                  : null}
              </span>
            ) : failed ? (
              <span className="flex items-center gap-1 text-[10px] text-muted-foreground">
                <ArrowClockwise size={11} /> Retry
              </span>
            ) : !hasLines ? (
              // A span, not a button: the row itself starts the job, and a
              // button inside a button is invalid markup that swallows its own
              // click. The same note `SourceRowStatus` carries.
              <span className="flex items-center gap-1 text-[10px] text-muted-foreground">
                <Sparkle size={11} /> Enhance
              </span>
            ) : value === "enhanced" ? (
              <Check size={13} weight="bold" />
            ) : null
          }
        />
      </PopoverContent>
    </Popover>
  );
}

/** One option: its name, what it is, and what the row is currently offering —
 *  a tick, a spinner, or the job. `SourceSwitcher`'s row without the icon,
 *  which here would only be two glyphs for two kinds of text. */
function Row({
  label,
  hint,
  hintClass,
  active,
  disabled,
  status,
  onClick,
}: {
  label: string;
  hint: string;
  hintClass?: string;
  active: boolean;
  disabled?: boolean;
  status: ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      title={hint}
      className={cn(
        "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors",
        "hover:bg-accent disabled:pointer-events-none disabled:opacity-50",
        active && "bg-accent",
      )}
    >
      <span className="min-w-0 flex-1">
        <span className="block text-[11.5px] font-medium leading-tight text-foreground">
          {label}
        </span>
        <span
          className={cn(
            "block truncate text-[10px] leading-tight text-muted-foreground",
            hintClass,
          )}
        >
          {hint}
        </span>
      </span>
      {status}
    </button>
  );
}
