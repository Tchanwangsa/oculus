import { useEffect, useRef, useState } from "react";
import { ArrowsOutSimple, X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

interface PeekPanelProps {
  title: string;
  /** Notion's expand: promote the peek to a full page in its own tab. */
  onExpand?: () => void;
  onClose: () => void;
  /** Right-aligned header controls (e.g. the PDF ↔ Markdown toggle). */
  actions?: React.ReactNode;
  children: React.ReactNode;
}

/**
 * Notion-style side peek: slides over the right of the whole page (its
 * positioned ancestor is AppLayout's main, so it spans from under the top tab
 * strip to the bottom), leaving the page behind visible and clickable.
 * Escape closes.
 */
export function PeekPanel({ title, onExpand, onClose, actions, children }: PeekPanelProps) {
  // Slide-out needs the panel alive for the animation, so closing is a
  // two-step: flag → animate → real onClose. Opening animates via mount.
  const [closing, setClosing] = useState(false);
  const closeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const requestClose = () => {
    if (closing) return;
    setClosing(true);
    closeTimer.current = setTimeout(onClose, 180);
  };

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") requestClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [onClose, closing]);

  // Clear a pending close only when the panel truly unmounts — clearing it on
  // effect re-runs (closing flipping) would cancel the onClose it just armed,
  // leaving the panel hidden but mounted and the peek store stuck open.
  useEffect(() => {
    return () => {
      if (closeTimer.current) clearTimeout(closeTimer.current);
    };
  }, []);

  return (
    <div
      role="dialog"
      aria-label={title}
      className={cn(
        "absolute inset-y-0 right-0 z-20 flex flex-col bg-background border-l border-border shadow-[-8px_0_24px_-12px_rgba(0,0,0,0.18)]",
        closing
          ? "animate-out slide-out-to-right-full fill-mode-forwards duration-200 ease-in"
          : "animate-in slide-in-from-right-full duration-200 ease-out",
      )}
      style={{ width: "min(760px, 85%)" }}
    >
      {/* Controls sit together top-left, Notion-style. */}
      <div className="h-10 shrink-0 flex items-center gap-0.5 px-2 border-b border-border-subtle">
        {onExpand && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-xs"
                onClick={onExpand}
                aria-label="Open as full page"
                className="text-muted-foreground hover:text-foreground"
              >
                <ArrowsOutSimple size={14} />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Open as full page</TooltipContent>
          </Tooltip>
        )}

        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon-xs"
              onClick={requestClose}
              aria-label="Close"
              className="text-muted-foreground hover:text-foreground"
            >
              <X size={14} />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Close (Esc)</TooltipContent>
        </Tooltip>

        <span className="flex-1 min-w-0 truncate text-[12px] font-medium text-foreground px-1.5">
          {title}
        </span>

        {actions}
      </div>

      <div className="flex-1 min-h-0 overflow-hidden flex flex-col">{children}</div>
    </div>
  );
}
