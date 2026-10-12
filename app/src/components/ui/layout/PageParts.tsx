import { Skeleton } from "../skeleton";
import type { ReactNode } from "react";
import { CircleNotch } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

/** A flat, hairline-divided list of rows without a surrounding card. */
export function ListCard({ className, children }: { className?: string; children: ReactNode }) {
  return (
    <div className={cn("divide-y divide-border-subtle", className)}>
      {children}
    </div>
  );
}

/** A spinner and "Loading…" centred in the space a view will fill. */
export function LoadingFill() {
  return (
    <div className="h-full flex items-center justify-center gap-2 text-muted-foreground">
      <CircleNotch size={16} className="animate-spin" />
      <span className="text-sm">Loading…</span>
    </div>
  );
}

/** A dashed drop-zone card for a list with nothing in it yet. */
export function EmptyState({
  icon,
  title,
  body,
  children,
}: {
  icon: ReactNode;
  title: string;
  body: string;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-border px-6 py-14 text-center">
      {icon}
      <div className="space-y-1">
        <p className="text-sm text-foreground">{title}</p>
        <p className="text-[12px] text-muted-foreground">{body}</p>
      </div>
      {children}
    </div>
  );
}

/** Fixed placeholder rows while a page's data is loading. */
export function SkeletonRows({ count, rowClassName = "h-8", className }: {
  count: number;
  rowClassName?: string;
  className?: string;
}) {
  return (
    <div className={cn("space-y-2", className)}>
      {Array.from({ length: count }, (_, i) => (
        <Skeleton key={i} className={cn("w-full", rowClassName)} />
      ))}
    </div>
  );
}
