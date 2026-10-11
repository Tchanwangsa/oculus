import { useState } from "react";
import {
  Check,
  CircleNotch,
  DownloadSimple,
  PictureInPicture,
  Rectangle,
  Rows,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { Layout } from "@/stores/lectures/playerPrefsStore";
import { glassPanel, type SourceState } from "@/components/lectures/SourceControls";

const LAYOUTS: { value: Layout; label: string; hint: string; Icon: typeof Rectangle }[] = [
  { value: "single", label: "One screen", hint: "Either source, full frame", Icon: Rectangle },
  { value: "pip", label: "Picture in picture", hint: "Drag and resize the inset", Icon: PictureInPicture },
  { value: "stack", label: "Stacked", hint: "Both, split top and bottom", Icon: Rows },
];

const LAYOUT_ICON: Record<Layout, typeof Rectangle> = {
  single: Rectangle,
  pip: PictureInPicture,
  stack: Rows,
};

/**
 * How the two sources share the frame. Two-frame layouts stay listed but
 * disabled until the second stream is on disk, with its download at the foot.
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
  // Closes on a pick, like the switcher; the download row keeps it open.
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
