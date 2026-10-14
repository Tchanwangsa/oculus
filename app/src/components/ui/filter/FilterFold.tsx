import { useState, type ReactNode } from "react";
import { CaretRight } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";

/**
 * A collapsed group at the foot of a filter list ("Archived (3)"). `forceOpen`
 * keeps it open while the current selection is inside, so a fold never hides
 * the choice.
 */
export function FilterFold({
  label,
  count,
  forceOpen,
  children,
}: {
  label: string;
  count: number;
  forceOpen: boolean;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const shown = open || forceOpen;
  return (
    <Collapsible open={shown} onOpenChange={setOpen} className="mt-1">
      <CollapsibleTrigger className="flex w-full cursor-pointer items-center gap-1.5 rounded-md px-2 py-1.5 text-left text-[11px] font-medium text-muted-foreground transition-colors hover:text-foreground">
        <CaretRight size={9} className={cn("shrink-0 transition-transform will-change-transform", shown && "rotate-90")} />
        {label} ({count})
      </CollapsibleTrigger>
      <CollapsibleContent>{children}</CollapsibleContent>
    </Collapsible>
  );
}
