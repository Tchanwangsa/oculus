import { useRef } from "react";
import { Info } from "@phosphor-icons/react";
import { Popover, PopoverAnchor, PopoverContent } from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { GAP, PAD } from "./layout";
import { ThreadInfoCard, type ThreadInfoProps } from "./ThreadInfoCard";
import type { ThreadInfoState } from "./useThreadInfo";

/**
 * The button at the end of the chat header. With room in the pane it shows
 * or hides the floating card (remembered); without, it opens the same card
 * as its popover, aligned to the pane's right edge so it stays over the chat.
 */
export function ThreadInfoButton({ info, ...card }: ThreadInfoProps & { info: ThreadInfoState }) {
  const button = useRef<HTMLButtonElement>(null);
  const pressed = info.floats ? info.shown : info.popoverOpen;
  return (
    <Popover open={!info.floats && info.popoverOpen} onOpenChange={info.setPopoverOpen}>
      <Tooltip>
        <PopoverAnchor asChild>
          <TooltipTrigger asChild>
            <button
              ref={button}
              type="button"
              aria-label={info.floats ? (info.shown ? "Hide thread info" : "Show thread info") : "Thread info"}
              {...(info.floats ? { "aria-pressed": info.shown } : { "aria-expanded": info.popoverOpen, "aria-haspopup": "dialog" as const })}
              onClick={info.toggle}
              className={cn(
                "flex h-8 w-8 shrink-0 items-center justify-center rounded-md transition-colors",
                pressed
                  ? "bg-accent text-foreground"
                  : "text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground",
              )}
            >
              <Info size={16} />
            </button>
          </TooltipTrigger>
        </PopoverAnchor>
        <TooltipContent side="bottom">Thread info</TooltipContent>
      </Tooltip>
      <PopoverContent
        align="end"
        side="bottom"
        // Where the floating card sits: `GAP` under the header, `GAP` from
        // the pane's edge (the button sits `PAD` in).
        sideOffset={GAP + 8}
        alignOffset={GAP - PAD}
        collisionPadding={GAP}
        // Focus goes to the card, not its first row, which would look hovered.
        onOpenAutoFocus={(e) => {
          e.preventDefault();
          (e.currentTarget as HTMLElement).querySelector<HTMLElement>('[role="region"]')?.focus();
        }}
        // The button toggles it; a press there is not a press outside.
        onInteractOutside={(e) => {
          if (button.current?.contains(e.target as Node)) e.preventDefault();
        }}
        className="w-auto rounded-2xl border-0 bg-transparent p-0 shadow-none"
      >
        <ThreadInfoCard {...card} onHeight={info.onCardHeight} onOpened={() => info.setPopoverOpen(false)} />
      </PopoverContent>
    </Popover>
  );
}
