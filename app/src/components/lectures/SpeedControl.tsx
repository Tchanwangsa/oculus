import { Minus, Plus, Speedometer } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Slider } from "@/components/ui/slider";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import {
  SPEED_MAX,
  SPEED_MIN,
  SPEED_PRESETS,
  SPEED_STEP,
  clampSpeed,
} from "@/stores/playerPrefsStore";

/** `1×`, `1.75×` — trailing zeros trimmed so a badge never reads `1.50×`. */
export const fmtSpeed = (s: number) => `${Number(s.toFixed(2))}×`;

function StepButton({
  label,
  onClick,
  disabled,
  children,
}: {
  label: string;
  onClick: () => void;
  disabled: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "inline-flex size-6 shrink-0 items-center justify-center rounded-full",
        "bg-white/15 text-white transition-colors",
        "hover:bg-white/25 disabled:opacity-40 disabled:pointer-events-none",
      )}
    >
      {children}
    </button>
  );
}

/**
 * Playback speed, YouTube's shape: a continuous 0.05 slider between −/+ nudges,
 * with the common speeds as presets underneath.
 *
 * The trigger is an icon plus a **fixed-width** badge. It used to be the number
 * alone in a hug-width pill, which changed width on every step — and since it
 * sits in a row of controls, every neighbour shifted with it.
 */
export function SpeedControl({
  speed,
  onChange,
  onOpenChange,
}: {
  speed: number;
  onChange: (speed: number) => void;
  /** The bar this sits on hides itself; an open panel has to pin it. */
  onOpenChange?: (open: boolean) => void;
}) {
  const nudge = (delta: number) => onChange(clampSpeed(speed + delta));

  return (
    <Popover onOpenChange={onOpenChange}>
      <Tooltip>
        <TooltipTrigger asChild>
          <PopoverTrigger
            className={cn(
              "h-8 shrink-0 rounded-full pl-2 pr-2.5 flex items-center gap-1",
              "text-white/85 transition-colors",
              "hover:bg-white/15 hover:text-white",
              "data-[state=open]:bg-white/20 data-[state=open]:text-white",
            )}
            aria-label={`Playback speed: ${fmtSpeed(speed)}`}
          >
            <Speedometer size={17} />
            <span className="w-[34px] text-center text-[11px] font-medium tabular-nums">
              {fmtSpeed(speed)}
            </span>
          </PopoverTrigger>
        </TooltipTrigger>
        <TooltipContent>Playback speed</TooltipContent>
      </Tooltip>

      <PopoverContent
        side="top"
        align="end"
        className={cn(
          // Dark glass, not the app's `popover` white: this panel floats over
          // the frame like the bar that opened it, so it takes the bar's fixed
          // white-on-frame palette and lets the slide read through it.
          "w-60 p-2.5 border-white/15 bg-black/55 text-white shadow-black/50",
          "backdrop-blur-xl backdrop-saturate-150",
          // The shared Slider styles its own track for a themed surface; on
          // glass that near-white grey is a bright bar. Match the scrub bar's.
          "[&_[data-slot=slider-track]]:bg-white/25",
        )}
      >
        <div className="flex items-baseline justify-between">
          <span className="text-[11px] font-medium text-white/60">
            Playback speed
          </span>
          <span className="text-[11px] font-medium tabular-nums">
            {fmtSpeed(speed)}
          </span>
        </div>

        <div className="mt-2 flex items-center gap-2">
          <StepButton
            label="Slower"
            onClick={() => nudge(-SPEED_STEP)}
            disabled={speed <= SPEED_MIN}
          >
            <Minus size={12} weight="bold" />
          </StepButton>

          <Slider
            min={SPEED_MIN}
            max={SPEED_MAX}
            step={SPEED_STEP}
            value={[speed]}
            onValueChange={([s]) => onChange(clampSpeed(s))}
            aria-label="Playback speed"
          />

          <StepButton
            label="Faster"
            onClick={() => nudge(SPEED_STEP)}
            disabled={speed >= SPEED_MAX}
          >
            <Plus size={12} weight="bold" />
          </StepButton>
        </div>

        {/* A grid, not a wrapping flex row: equal columns keep the presets on
            one line whatever they read, and none of them can drop below. */}
        <div className="mt-2 grid grid-cols-6 gap-1">
          {SPEED_PRESETS.map((s) => (
            <button
              key={s}
              type="button"
              onClick={() => onChange(s)}
              className={cn(
                "h-5 rounded-full text-[10px] font-medium tabular-nums transition-colors",
                s === speed
                  ? "bg-primary text-primary-foreground"
                  : "bg-white/12 text-white/70 hover:bg-white/22 hover:text-white",
              )}
            >
              {s}
            </button>
          ))}
        </div>
      </PopoverContent>
    </Popover>
  );
}
