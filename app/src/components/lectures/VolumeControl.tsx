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
 * A mute button with a horizontal slider that grows out of it on hover, focus
 * or drag. It sits left of the timestamp so widening moves nothing else.
 * Muting keeps the stored `volume` as the level to return to, while the slider
 * shows zero; dragging it up unmutes.
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

  // A drag that left the bar fires nothing else there, so the release is
  // heard on the window.
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

      {/* The wrapper animates width; the slider keeps its own so it isn't
          squeezed. */}
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
