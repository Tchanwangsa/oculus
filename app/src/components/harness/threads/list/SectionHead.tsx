import { CaretRight } from "@phosphor-icons/react";
import { SIDE_NAV_FOLDS } from "@/components/ui/layout/SideNav";
import { cn } from "@/lib/utils";
import { SECTION_HEAD } from "../rowType";

/** A Status-view section's fold: its name, count and caret (Working, Completed). */
export function SectionHead({
  label,
  count,
  open,
  onOpen,
  collapsed,
}: {
  label: string;
  count: number;
  open: boolean;
  onOpen: (open: boolean) => void;
  collapsed: boolean;
}) {
  return (
    <button
      type="button"
      aria-expanded={open}
      tabIndex={collapsed ? -1 : undefined}
      onClick={() => onOpen(!open)}
      className={cn(
        "flex w-full items-center gap-2 px-2.5 text-left text-[12.5px] font-medium text-muted-foreground transition-colors hover:text-foreground",
        SECTION_HEAD,
        SIDE_NAV_FOLDS,
      )}
    >
      <span className="flex-1">{label}</span>
      <span className="font-normal tabular-nums opacity-80">{count}</span>
      <CaretRight
        size={12}
        className={cn("shrink-0 opacity-70 transition-transform will-change-transform", open && "rotate-90")}
      />
    </button>
  );
}
