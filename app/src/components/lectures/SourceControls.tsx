import { useState } from "react";
import {
  CaretDown,
  Check,
  CircleNotch,
  DownloadSimple,
  Monitor,
  VideoCamera,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import type { SourceNum } from "@/lib/db";

/**
 * The per-frame source switcher for two-stream captures. It floats over the
 * video, so it uses `SpeedControl`'s fixed glass palette.
 */

/** What the player knows about one stream: on disk, or on its way there. */
export interface SourceState {
  /** A downloaded file exists, so this source can be shown. */
  ready: boolean;
  /** A download is running for it. */
  busy: boolean;
  percent: number;
  phase: string;
}

export type SourceStates = Record<SourceNum, SourceState>;

export const SOURCES: SourceNum[] = [1, 2];

/** Echo360 numbers streams rather than naming them, so the label is the
 *  number and the icon carries only the usual meaning. */
const SOURCE_LABEL: Record<SourceNum, string> = {
  1: "Source 1",
  2: "Source 2",
};

/** The usual meaning of each number — a hint, not a fact. */
export const SOURCE_HINT: Record<SourceNum, string> = {
  1: "Presenter screen",
  2: "Room camera",
};

function SourceIcon({ source, size = 14 }: { source: SourceNum; size?: number }) {
  return source === 1 ? <Monitor size={size} /> : <VideoCamera size={size} />;
}

/** The floating panels' shared glass look. */
export const glassPanel = cn(
  "border-white/15 bg-black/55 text-white shadow-black/50",
  "backdrop-blur-xl backdrop-saturate-150",
);

/** The trailing state of a source row: a tick, a spinner, or a download. */
function SourceRowStatus({ state, active }: { state: SourceState; active: boolean }) {
  if (state.busy) {
    return (
      <span className="flex items-center gap-1 text-[10.5px] tabular-nums text-white/60">
        <CircleNotch size={12} className="animate-spin" />
        {state.phase === "trimming" ? "Trimming" : `${state.percent}%`}
      </span>
    );
  }
  if (!state.ready) {
    return (
      // A span: the row is the button, and nested buttons swallow the click.
      <span className="flex items-center gap-1 text-[10.5px] text-white/60">
        <DownloadSimple size={12} /> Download
      </span>
    );
  }
  return active ? <Check size={13} weight="bold" /> : <span className="size-[13px]" />;
}

/**
 * Which stream this frame shows; picking one in either frame swaps the pair.
 * Undownloaded sources are listed, since this is where the camera is found.
 */
export function SourceSwitcher({
  active,
  states,
  onSelect,
  onDownload,
  onOpenChange,
}: {
  active: SourceNum;
  states: SourceStates;
  onSelect: (source: SourceNum) => void;
  onDownload: (source: SourceNum) => void;
  /** The frame this sits on hides it on pointer-out; an open panel pins it. */
  onOpenChange?: (open: boolean) => void;
}) {
  // Controlled so a pick closes it; a download row leaves it open to show
  // progress.
  const [open, setOpen] = useState(false);
  const change = (next: boolean) => {
    setOpen(next);
    onOpenChange?.(next);
  };

  return (
    <Popover open={open} onOpenChange={change}>
      <PopoverTrigger
        className={cn(
          "flex h-7 items-center gap-1.5 rounded-full pl-2 pr-2 text-[11px] font-medium",
          "border border-white/15 bg-black/55 text-white/85 backdrop-blur-xl",
          "transition-colors hover:bg-black/70 hover:text-white",
          "data-[state=open]:bg-black/75 data-[state=open]:text-white",
        )}
        aria-label={`Showing ${SOURCE_LABEL[active]} — change source`}
      >
        <SourceIcon source={active} size={13} />
        {SOURCE_LABEL[active]}
        <CaretDown size={9} weight="bold" className="text-white/50" />
      </PopoverTrigger>

      <PopoverContent side="bottom" align="start" className={cn("w-52 p-1", glassPanel)}>
        {SOURCES.map((s) => (
          <button
            key={s}
            type="button"
            onClick={() => {
              if (!states[s].ready) return onDownload(s);
              onSelect(s);
              change(false);
            }}
            disabled={states[s].busy}
            className={cn(
              "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors",
              "hover:bg-white/12 disabled:pointer-events-none",
              s === active && "bg-white/10",
            )}
          >
            <SourceIcon source={s} />
            <span className="min-w-0 flex-1">
              <span className="block text-[11.5px] font-medium leading-tight">
                {SOURCE_LABEL[s]}
              </span>
              <span className="block text-[10px] leading-tight text-white/45">
                {SOURCE_HINT[s]}
              </span>
            </span>
            <SourceRowStatus state={states[s]} active={s === active} />
          </button>
        ))}
      </PopoverContent>
    </Popover>
  );
}
