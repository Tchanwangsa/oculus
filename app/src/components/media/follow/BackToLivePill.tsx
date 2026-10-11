import type { RefObject } from "react";
import { ArrowLineDown, ArrowLineUp } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { RING_STROKE } from "@/components/media/follow/constants";

interface BackToLivePillProps {
  show: boolean;
  /** The playing row is above the viewport's middle. */
  liveAbove: boolean;
  onBackToLive: () => void;
  pillRef: RefObject<HTMLButtonElement | null>;
  ringRef: RefObject<SVGRectElement | null>;
  pill: { w: number; h: number };
  ringW: number;
  ringH: number;
  ringLen: number;
}

/** Scrolled away from the playing row — offers the way back, with a ring that
 *  drains over the idle window before the list re-syncs itself. */
export function BackToLivePill({
  show: showBackToLive,
  liveAbove,
  onBackToLive,
  pillRef,
  ringRef,
  pill,
  ringW,
  ringH,
  ringLen,
}: BackToLivePillProps) {
  return (
    <div
      className={cn(
        "pointer-events-none absolute inset-x-0 bottom-3 z-20 flex justify-center transition-opacity will-change-[opacity] duration-200",
        showBackToLive ? "opacity-100" : "opacity-0",
      )}
    >
      <button
        ref={pillRef}
        onClick={onBackToLive}
        tabIndex={showBackToLive ? 0 : -1}
        aria-hidden={!showBackToLive}
        className={cn(
          "pointer-events-auto relative h-6 pl-2 pr-2.5 rounded-full flex items-center gap-1",
          "bg-brand text-brand-foreground text-[11px] font-medium",
          "shadow-md shadow-black/15 hover:bg-brand-hover transition-colors",
          !showBackToLive && "pointer-events-none",
        )}
      >
        {/* Drains over the idle window before the list re-syncs itself. */}
        {pill.w > 0 && (
          <svg
            aria-hidden
            viewBox={`0 0 ${pill.w} ${pill.h}`}
            className="pointer-events-none absolute inset-0 h-full w-full text-brand-foreground/70"
          >
            <rect
              ref={ringRef}
              x={RING_STROKE / 2}
              y={RING_STROKE / 2}
              width={ringW}
              height={ringH}
              rx={ringH / 2}
              fill="none"
              stroke="currentColor"
              strokeWidth={RING_STROKE}
              strokeDasharray={ringLen}
            />
          </svg>
        )}
        {liveAbove ? (
          <ArrowLineUp size={11} weight="bold" />
        ) : (
          <ArrowLineDown size={11} weight="bold" />
        )}
        Back to live
      </button>
    </div>
  );
}
