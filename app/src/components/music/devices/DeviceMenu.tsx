import { useState } from "react";
import { Devices } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { MusicSnapshot } from "@/lib/music";
import { cn } from "@/lib/utils";
import { OutputDevices } from "./OutputDevices";
import { SpotifyDevices } from "./SpotifyDevices";

/** The player's device button: Spotify's Connect devices and this Mac's
 *  outputs, in one menu. Brand-tinted while another device plays. Both lists
 *  are fetched each time it opens, never on a timer. */
export function DeviceMenu({ snapshot }: { snapshot: MusicSnapshot }) {
  const [open, setOpen] = useState(false);
  const elsewhere = !snapshot.active && snapshot.playing_on !== null;
  const label = elsewhere ? `Playing on ${snapshot.playing_on}` : "Devices";

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <Tooltip>
        <TooltipTrigger asChild>
          <PopoverTrigger asChild>
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label={label}
              disabled={!snapshot.connected}
              className={cn("shrink-0", elsewhere ? "text-brand hover:text-brand" : "text-muted-foreground")}
            >
              <Devices size={15} />
            </Button>
          </PopoverTrigger>
        </TooltipTrigger>
        {!open && <TooltipContent>{label}</TooltipContent>}
      </Tooltip>
      <PopoverContent align="end" className="flex w-64 flex-col gap-1.5 p-1.5">
        <SpotifyDevices onMoved={() => setOpen(false)} />
        <div className="border-t border-border-subtle pt-1.5">
          <OutputDevices chosen={snapshot.output} />
        </div>
      </PopoverContent>
    </Popover>
  );
}
