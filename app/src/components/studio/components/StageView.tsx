import { forwardRef } from "react";
import { CircleNotch, ImageBroken } from "@phosphor-icons/react";

import type { StageFrame } from "@/hooks/studio/useStageFrame";
import { cn } from "@/lib/utils";

/**
 * The component's frame at the playhead. The last frame that drew stays up
 * (dimmed) while a request fails, with the backend's sentence under it; a
 * request that has not drawn yet shows the empty frame.
 */
export const StageView = forwardRef<HTMLDivElement, { frame: StageFrame; problem: string | null; alt: string }>(
  function StageView({ frame, problem, alt }, ref) {
    const message = problem ?? frame.error;
    return (
      <div className="space-y-2">
        <div
          ref={ref}
          className="relative aspect-video w-full overflow-hidden rounded-xl bg-secondary ring-1 ring-border"
        >
          {frame.url && !problem && (
            <img
              src={frame.url}
              alt={alt}
              draggable={false}
              className={cn("size-full object-contain", frame.error && "opacity-40")}
            />
          )}
          {!frame.url && !message && (
            <div className="flex size-full items-center justify-center">
              <CircleNotch size={18} className="animate-spin text-muted-foreground" aria-label="Drawing" />
            </div>
          )}
          {!frame.url && message && (
            <div className="flex size-full items-center justify-center">
              <ImageBroken size={22} weight="light" className="text-muted-foreground" />
            </div>
          )}
        </div>
        {message && (
          <p data-selectable className="text-[12.5px] leading-snug text-destructive">
            {message}
          </p>
        )}
      </div>
    );
  },
);
