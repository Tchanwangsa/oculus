import { useState, type ReactNode } from "react";
import { CaretUpDown, Check } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";

/**
 * A toolbar filter: a pill naming the current choice, brand-tinted while it
 * narrows the list, over a popover of rows. `children` is a render prop so a
 * single-choice row can close the popover.
 */
export function FilterPopover({
  label,
  active,
  icon,
  className,
  triggerClassName,
  children,
}: {
  label: string;
  active: boolean;
  icon?: ReactNode;
  /** The popover's width and padding, over `w-56 p-1.5`. */
  className?: string;
  /** Over the pill's `h-7 px-2.5 text-xs`, e.g. to match the buttons beside it. */
  triggerClassName?: string;
  children: (close: () => void) => ReactNode;
}) {
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          size="sm"
          className={cn(
            "h-7 shrink-0 gap-1.5 px-2.5 text-xs font-normal",
            active ? "border-brand/40 text-brand" : "text-foreground",
            triggerClassName,
          )}
        >
          {icon}
          <span className="max-w-32 truncate">{label}</span>
          <CaretUpDown size={11} className="shrink-0 text-muted-foreground/60" />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="start" className={cn("w-56 p-1.5", className)}>
        {children(() => setOpen(false))}
      </PopoverContent>
    </Popover>
  );
}

/** A single-choice row: the picked one is filled and checked. */
export function FilterRow({
  label,
  hint,
  count,
  icon,
  selected,
  onPick,
  disabled,
  className,
}: {
  label: string;
  hint?: string;
  count?: number;
  icon?: ReactNode;
  selected: boolean;
  onPick: () => void;
  disabled?: boolean;
  className?: string;
}) {
  return (
    <button
      type="button"
      title={hint ?? label}
      onClick={onPick}
      disabled={disabled}
      className={cn(
        "flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-[12.5px] outline-none transition-colors focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50",
        selected
          ? "bg-accent text-foreground"
          : "text-muted-foreground hover:bg-accent hover:text-foreground",
        className,
      )}
    >
      {icon}
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {selected && <Check size={12} weight="bold" className="shrink-0 text-brand" />}
      {count != null && <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">({count})</span>}
    </button>
  );
}
