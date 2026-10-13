import { useState } from "react";
import { ArrowCounterClockwise, DotsThree } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { cn } from "@/lib/utils";

import type { useReplaceFlow } from "./useReplaceFlow";

/**
 * The workspace header's overflow menu: Reset to scaffold, Ed's "Reset
 * attempt", asked first in Ed's own words. Disabled offline, the reason as
 * its tooltip.
 */
export function WorkspaceMenu({
  reset,
  offline,
}: {
  reset: ReturnType<typeof useReplaceFlow<true>>;
  /** Why Ed can't be reached now; null when online. */
  offline: string | null;
}) {
  const [open, setOpen] = useState(false);
  const disabled = offline != null || reset.busy;
  return (
    <>
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label="More workspace actions"
            title="More"
            className="shrink-0 text-muted-foreground hover:text-foreground"
          >
            <DotsThree size={16} weight="bold" />
          </Button>
        </PopoverTrigger>
        <PopoverContent
          align="end"
          className="w-48 p-1"
          // Closing into the dialog: its focus trap keeps the focus.
          onCloseAutoFocus={(e) => reset.asking != null && e.preventDefault()}
        >
          <span title={offline ?? undefined} className="block">
            <button
              type="button"
              disabled={disabled}
              onClick={() => {
                reset.ask(true);
                setOpen(false);
              }}
              className={cn(
                "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs text-destructive transition-colors",
                disabled ? "cursor-default opacity-50" : "cursor-pointer hover:bg-destructive/10",
              )}
            >
              <ArrowCounterClockwise size={12} className="shrink-0 opacity-70" aria-hidden />
              <span className="min-w-0 flex-1 truncate">Reset to scaffold</span>
            </button>
          </span>
        </PopoverContent>
      </Popover>
      <ConfirmDialog
        open={reset.asking != null}
        onCancel={reset.cancel}
        onConfirm={reset.confirm}
        title="Reset challenge attempt?"
        description="This will delete all of your files and restore the original set of files for this challenge. The challenge will be reloaded."
        confirmLabel="Reset"
        busyLabel="Resetting…"
        busy={reset.busy}
      />
    </>
  );
}
