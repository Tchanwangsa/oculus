import { Queue } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

/** The full player's queue button, beside the device button: swaps the
 *  library under the player for what plays next. Brand-tinted while open. */
export function QueueToggle({ open, onToggle }: { open: boolean; onToggle: () => void }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label="Queue"
          aria-pressed={open}
          onClick={onToggle}
          className={cn("shrink-0", open ? "text-brand hover:text-brand" : "text-muted-foreground")}
        >
          <Queue size={15} />
        </Button>
      </TooltipTrigger>
      <TooltipContent>{open ? "Hide queue" : "Show queue"}</TooltipContent>
    </Tooltip>
  );
}
