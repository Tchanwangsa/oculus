import type { ReactNode } from "react";
import { CaretDown } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

/** A foldable group inside a GridTable, keeping its rows under one header. */
export function TableGroup({ label, count, icon, open, onToggle, children }: {
  label: string;
  count: number;
  icon?: ReactNode;
  open: boolean;
  onToggle: () => void;
  children: ReactNode;
}) {
  return (
    <section aria-label={label}>
      <button type="button" aria-expanded={open} onClick={onToggle}
        className="flex h-8 w-full cursor-pointer items-center gap-2 border-b border-border-subtle bg-surface/50 px-5 text-left outline-none transition-colors hover:bg-surface focus-visible:ring-[3px] focus-visible:ring-inset focus-visible:ring-ring/25">
        <CaretDown size={10} weight="bold" className={cn("shrink-0 text-muted-foreground transition-transform will-change-transform", !open && "-rotate-90")} />
        {icon}
        <span className="min-w-0 truncate text-xs font-medium text-foreground">{label}</span>
        <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">{count}</span>
      </button>
      {open && <div className="divide-y divide-border-subtle border-b border-border-subtle">{children}</div>}
    </section>
  );
}
