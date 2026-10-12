import type { ReactNode } from "react";
import { CaretRight } from "@phosphor-icons/react";

import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { cn } from "@/lib/utils";

/**
 * A titled block that folds: a full-width header (caret, title, a count at the
 * end) over content that is unmounted while closed. The caller owns `open`, so
 * it can remember it. `marked` lights a brand dot beside the title, for a
 * folded section that still holds a change.
 */
export function CollapsibleSection({
  title,
  open,
  onOpenChange,
  count,
  marked = false,
  className,
  children,
}: {
  title: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Shown at the header's end, quietly: how many things are inside. */
  count?: ReactNode;
  marked?: boolean;
  className?: string;
  children: ReactNode;
}) {
  return (
    <Collapsible
      open={open}
      onOpenChange={onOpenChange}
      className={cn("border-b border-border-subtle last:border-b-0", className)}
    >
      <CollapsibleTrigger className="flex h-9 w-full cursor-pointer items-center gap-1.5 rounded-md text-left outline-none focus-visible:ring-[3px] focus-visible:ring-ring/25">
        <CaretRight
          size={11}
          weight="bold"
          className={cn("shrink-0 text-muted-foreground transition-transform will-change-transform", open && "rotate-90")}
        />
        <span className="min-w-0 truncate text-[12px] font-medium text-foreground">{title}</span>
        {marked && <span role="img" aria-label="Has changes" className="size-1.5 shrink-0 rounded-full bg-brand" />}
        <span className="min-w-0 flex-1" />
        {count != null && <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">{count}</span>}
      </CollapsibleTrigger>
      <CollapsibleContent className="pb-3.5">{children}</CollapsibleContent>
    </Collapsible>
  );
}
