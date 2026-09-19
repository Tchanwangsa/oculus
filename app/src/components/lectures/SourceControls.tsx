import { useState } from "react";
import {
  CaretDown,
  Check,
  CircleNotch,
  DownloadSimple,
  Monitor,
  PictureInPicture,
  Rectangle,
  Rows,
  VideoCamera,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { SourceNum } from "@/lib/db";
import type { Layout } from "@/stores/playerPrefsStore";

/**
 * The two controls that exist only because a capture can be two recordings:
 * which stream a frame is showing, and how the two frames share the player.
 *
 * Both float over the video, so they take the control bar's fixed
 * white-on-frame palette rather than the app's themed surfaces — see
 * `SpeedControl`, whose glass panel these match.
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

/**
 * Echo360 names the streams by number, not by content, and which camera points
 * where is the theatre's business — so the label is the number, and the icon
 * only carries the *usual* meaning (Presenter screen, room camera).
 */
export const SOURCE_LABEL: Record<SourceNum, string> = {
  1: "Source 1",
  2: "Source 2",
};

/** The *usual* meaning of each number, which is as much as anyone can say —
 *  the moment a dock message carries hedges it the same way. */
export const SOURCE_HINT: Record<SourceNum, string> = {
  1: "Presenter screen",
  2: "Room camera",
};

function SourceIcon({ source, size = 14 }: { source: SourceNum; size?: number }) {
  return source === 1 ? <Monitor size={size} /> : <VideoCamera size={size} />;
}

const glassPanel = cn(
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
      // A span, not a button: the row itself starts the download, and a button
      // inside a button is invalid markup that swallows its own click.
      <span className="flex items-center gap-1 text-[10.5px] text-white/60">
        <DownloadSimple size={12} /> Download
      </span>
    );
  }
  return active ? <Check size={13} weight="bold" /> : <span className="size-[13px]" />;
}

/**
 * Which stream this frame shows. One per frame, at its top-left, revealed on
 * hover — a two-frame layout has two of these and picking a source in one puts
 * the other stream in the other frame, so the pair always shows both.
 *
 * A source that has not been downloaded is still listed: this is where you
 * find out the camera exists, so it is also where you ask for it.
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
  // Open in state rather than left to Radix, because picking a source is the
  // end of the errand: you came here to switch, and the panel has nothing
  // left to say. Asking for a *download* is the one row that does — the
  // percentage is in it — so that one leaves the panel up.
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

// ── Layout ───────────────────────────────────────────────────────────────────

const LAYOUTS: { value: Layout; label: string; hint: string; Icon: typeof Rectangle }[] = [
  { value: "single", label: "One screen", hint: "Either source, full frame", Icon: Rectangle },
  { value: "pip", label: "Picture in picture", hint: "Drag and resize the inset", Icon: PictureInPicture },
  { value: "stack", label: "Stacked", hint: "Both, split top and bottom", Icon: Rows },
];

export const LAYOUT_ICON: Record<Layout, typeof Rectangle> = {
  single: Rectangle,
  pip: PictureInPicture,
  stack: Rows,
};

/**
 * How the two sources share the frame. Lives on the control bar rather than on
 * a frame, because it is about the player as a whole; the per-frame switcher
 * above is about one picture.
 *
 * Both two-frame layouts need the second stream on disk, so when it isn't the
 * options stay listed but disabled, with the download as the way through. The
 * alternative — hiding them — would mean the feature only appears once you
 * have already found the download somewhere else.
 */
export function LayoutControl({
  layout,
  onChange,
  second,
  onDownloadSecond,
  onOpenChange,
}: {
  layout: Layout;
  onChange: (layout: Layout) => void;
  /** The stream the two-frame layouts need. */
  second: SourceState;
  onDownloadSecond: () => void;
  onOpenChange?: (open: boolean) => void;
}) {
  const Trigger = LAYOUT_ICON[layout];
  const dual = second.ready;
  // Closes on a pick, for the same reason as the switcher above; the download
  // row at the foot keeps it open so its progress stays in sight.
  const [open, setOpen] = useState(false);
  const change = (next: boolean) => {
    setOpen(next);
    onOpenChange?.(next);
  };

  return (
    <Popover open={open} onOpenChange={change}>
      <Tooltip>
        <TooltipTrigger asChild>
          <PopoverTrigger
            className={cn(
              "inline-flex size-8 shrink-0 items-center justify-center rounded-full",
              "text-white/85 transition-colors hover:bg-white/15 hover:text-white",
              "data-[state=open]:bg-white/20 data-[state=open]:text-white",
            )}
            aria-label="Screen layout"
          >
            <Trigger size={17} />
          </PopoverTrigger>
        </TooltipTrigger>
        <TooltipContent>Screen layout</TooltipContent>
      </Tooltip>

      <PopoverContent side="top" align="end" className={cn("w-56 p-1", glassPanel)}>
        {LAYOUTS.map(({ value, label, hint, Icon }) => {
          const disabled = value !== "single" && !dual;
          return (
            <button
              key={value}
              type="button"
              onClick={() => {
                onChange(value);
                change(false);
              }}
              disabled={disabled}
              className={cn(
                "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors",
                "hover:bg-white/12 disabled:pointer-events-none disabled:opacity-35",
                value === layout && "bg-white/10",
              )}
            >
              <Icon size={15} />
              <span className="min-w-0 flex-1">
                <span className="block text-[11.5px] font-medium leading-tight">{label}</span>
                <span className="block text-[10px] leading-tight text-white/45">{hint}</span>
              </span>
              {value === layout && <Check size={13} weight="bold" />}
            </button>
          );
        })}

        {!dual && (
          <button
            type="button"
            onClick={onDownloadSecond}
            disabled={second.busy}
            className={cn(
              "mt-1 flex w-full items-center gap-2 rounded-md border-t border-white/10 px-2 pb-1 pt-2",
              "text-[11px] text-white/70 transition-colors",
              "hover:text-white disabled:pointer-events-none",
            )}
          >
            {second.busy ? (
              <>
                <CircleNotch size={12} className="animate-spin" />
                <span className="tabular-nums">
                  {second.phase === "trimming"
                    ? "Trimming Source 2…"
                    : `Downloading Source 2… ${second.percent}%`}
                </span>
              </>
            ) : (
              <>
                <DownloadSimple size={12} />
                Download Source 2 to use these
              </>
            )}
          </button>
        )}
      </PopoverContent>
    </Popover>
  );
}
