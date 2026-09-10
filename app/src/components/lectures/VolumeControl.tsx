import { useEffect, useState } from "react";
import {
  SpeakerHigh,
  SpeakerLow,
  SpeakerNone,
  SpeakerSimpleX,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Slider } from "@/components/ui/slider";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { clampVolume } from "@/stores/playerPrefsStore";

/** The icon says the level, since a collapsed slider cannot. */
function VolumeIcon({
  volume,
  muted,
  size = 17,
}: {
  volume: number;
  muted: boolean;
  size?: number;
}) {
  if (muted) return <SpeakerSimpleX size={size} />;
  if (volume === 0) return <SpeakerNone size={size} />;
  if (volume < 0.5) return <SpeakerLow size={size} />;
  return <SpeakerHigh size={size} />;
}

/**
 * Volume, YouTube's shape: a speaker button that mutes, with a **horizontal**
 * slider that grows out of it on hover and collapses again when the pointer
 * leaves. It sits left of the timestamp so the widening only ever pushes the
 * time and nothing else — every button on the bar keeps its place.
 *
 * The slider stays out while it is being dragged, since a drag routinely
 * wanders off the strip it started on, and while it holds focus, so the
 * keyboard can reach it at all.
 *
 * Muting leaves the stored `volume` alone — it is the level to come back to —
 * but the slider still drops to zero while muted, because that is what the
 * ear hears; dragging it back up is itself the unmute.
 */
export function VolumeControl({
  volume,
  muted,
  onChange,
  onToggleMute,
  onDraggingChange,
}: {
  volume: number;
  muted: boolean;
  onChange: (volume: number) => void;
  onToggleMute: () => void;
  /** The bar this sits on hides itself; a drag in progress has to pin it. */
  onDraggingChange?: (dragging: boolean) => void;
}) {
  const [dragging, setDragging] = useState(false);

  // Radix captures the pointer for the drag, so the release lands on the
  // thumb wherever it happens — but a pointer that left the bar is not
  // coming back to fire anything else, so listen globally.
  useEffect(() => {
    if (!dragging) return;
    const end = () => {
      setDragging(false);
      onDraggingChange?.(false);
    };
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
    return () => {
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
    };
  }, [dragging, onDraggingChange]);

  return (
    <div
      className="group/volume flex items-center"
      data-dragging={dragging || undefined}
    >
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            onClick={onToggleMute}
            aria-label={muted ? "Unmute" : "Mute"}
            className={cn(
              "inline-flex size-8 shrink-0 items-center justify-center rounded-full",
              "text-white/85 transition-colors",
              "hover:bg-white/15 hover:text-white",
            )}
          >
            <VolumeIcon volume={volume} muted={muted} />
          </button>
        </TooltipTrigger>
        <TooltipContent>{muted ? "Unmute" : "Mute"}</TooltipContent>
      </Tooltip>

      {/* The width animates on the wrapper; the slider inside keeps its own
          so it slides out at full size instead of being squeezed into shape. */}
      <div
        className={cn(
          "overflow-hidden transition-[width,opacity] duration-150 ease-out",
          "w-0 opacity-0",
          "group-hover/volume:w-[74px] group-hover/volume:opacity-100",
          "group-focus-within/volume:w-[74px] group-focus-within/volume:opacity-100",
          "group-data-[dragging]/volume:w-[74px] group-data-[dragging]/volume:opacity-100",
        )}
      >
        <div className="w-[74px] px-2">
          <Slider
            min={0}
            max={1}
            step={0.01}
            value={[muted ? 0 : volume]}
            onPointerDown={() => {
              setDragging(true);
              onDraggingChange?.(true);
            }}
            onValueChange={([v]) => onChange(clampVolume(v))}
            aria-label="Volume"
            className={cn(
              "cursor-pointer",
              "[&_[data-slot=slider-track]]:h-[3px] [&_[data-slot=slider-track]]:bg-white/30",
              "[&_[data-slot=slider-range]]:bg-white",
              "[&_[data-slot=slider-thumb]]:size-3 [&_[data-slot=slider-thumb]]:border-0",
              "[&_[data-slot=slider-thumb]]:bg-white [&_[data-slot=slider-thumb]]:shadow-none",
              "[&_[data-slot=slider-thumb]]:ring-white/25",
            )}
          />
        </div>
      </div>
    </div>
  );
}
