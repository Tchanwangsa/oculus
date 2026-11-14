import { useState, type ReactNode } from "react";
import { CaretRight } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";

/**
 * A collapsed group at the foot of a filter list ("Archived (3)"). `defaultOpen`
 * reveals the current selection initially; the user can still close it. A
 * caller that must know whether the rows show (keyboard focus over them)
 * holds the state itself through `open` and `onOpenChange`.
 */
export function FilterFold({
  label,
  count,
  defaultOpen = false,
  open,
  onOpenChange,
  children,
}: {
  label: string;
  count: number;
  defaultOpen?: boolean;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  children: ReactNode;
}) {
  const [own, setOwn] = useState(defaultOpen);
  const shown = open ?? own;
  const setOpen = onOpenChange ?? setOwn;
  return (
    <Collapsible open={shown} onOpenChange={setOpen} className="mt-1">
      <CollapsibleTrigger className="flex w-full cursor-pointer items-center gap-1.5 rounded-md px-2 py-1.5 text-left text-[11px] font-medium text-muted-foreground outline-none transition-colors hover:text-foreground focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50">
        <CaretRight size={9} className={cn("shrink-0 transition-transform will-change-transform", shown && "rotate-90")} />
        {label} ({count})
      </CollapsibleTrigger>
      <CollapsibleContent>{children}</CollapsibleContent>
    </Collapsible>
  );
}
