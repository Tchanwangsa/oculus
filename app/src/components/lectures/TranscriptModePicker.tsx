import { useState, type ReactNode } from "react";
import { ArrowClockwise, CaretDown, Check, CircleNotch, Sparkle } from "@phosphor-icons/react";

import { cn } from "@/lib/utils";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { READING_PHASE_LABEL, type ReadingRunProgress } from "@/lib/lectures";
import type { ReadingStatus } from "@/hooks/useLectureReading";
import type { TranscriptMode } from "@/stores/playerPrefsStore";

const MODE_LABEL: Record<TranscriptMode, string> = {
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
  /** `reading_error`, shown verbatim: the only place a failed run with no
   *  lines surfaces. */
  error: string | null;
  progress: ReadingRunProgress | null;
  /** A run is in flight — Rust refuses the second call outright. */
  busy: boolean;
  /** The job watches the recording as well as reading the transcript. */
  downloaded: boolean;
  /** There is an enhanced copy; until then, picking Enhanced asks for one. */
  hasLines: boolean;
  onEnhance: (force: boolean) => void;
}

/**
 * Which register the Transcript tab is in, and — until an enhanced copy exists
 * — how it gets written: the source switcher's shape, where the row for what
 * is not there yet starts the job. Themed, not glass: it sits in the dock.
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
  // Controlled so any pick closes it — including Enhance, whose lines then
  // arrive in the list behind.
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
        {/* A run keeps showing here while you read the other register. */}
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
          // Why it can't be picked, or the failed run's own message.
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
              // A span: the row is the button, and nested buttons swallow the click.
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

/** One option: name, hint, and a tick, spinner or the job. */
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
