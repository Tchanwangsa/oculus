import type { ReactNode } from "react";
import { CaretRight } from "@phosphor-icons/react";
import { Popover, PopoverTrigger } from "@/components/ui/popover";

/** Where a submenu opens: beside its row, growing up from the row's bottom
 *  edge (the bar sits low on the page); Radix flips it left when the right
 *  has no room. */
export const SUBMENU_PLACEMENT = { side: "right", align: "end", sideOffset: 6, collisionPadding: 8 } as const;

/**
 * One row of the compact composer's settings menu: an icon, what is set, a
 * muted detail and a chevron, opening its own popover beside the menu.
 * `children` is that popover's `PopoverContent`, placed with
 * `SUBMENU_PLACEMENT`. The submenu portals out but stays in the menu's React
 * tree, so clicks inside it never count as outside the menu.
 */
export function SettingsSubmenu({
  open,
  onOpenChange,
  icon,
  label,
  value,
  title,
  children,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  icon: ReactNode;
  label: string;
  /** The muted detail after the label (a reasoning level, a usage figure). */
  value?: string | null;
  title?: string;
  children: ReactNode;
}) {
  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger asChild>
        <button
          type="button"
          title={title}
          className="flex w-full cursor-default items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs text-foreground outline-none transition-colors hover:bg-accent focus-visible:bg-accent data-[state=open]:bg-accent"
        >
          <span className="flex size-4 shrink-0 items-center justify-center text-muted-foreground">{icon}</span>
          <span className="min-w-0 truncate">{label}</span>
          {value && <span className="min-w-0 truncate text-muted-foreground">{value}</span>}
          <CaretRight className="ml-auto size-3 shrink-0 text-muted-foreground" aria-hidden />
        </button>
      </PopoverTrigger>
      {children}
    </Popover>
  );
}
