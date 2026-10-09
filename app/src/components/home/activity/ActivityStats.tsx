import type { ReactNode } from "react";
import { ArrowDown, ArrowUp, type Icon } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

/** One active-time figure under its icon and label, with a comparison or a
 *  note below; blank until the first read lands, so the row holds its height. */
export function Stat({
  icon: Glyph,
  label,
  value,
  detail,
}: {
  icon: Icon;
  label: string;
  value: string | null | undefined;
  detail: ReactNode;
}) {
  return (
    <div className="rounded-lg border border-border px-3.5 py-3">
      <p className="flex items-center gap-1.5 text-[11px] font-medium text-muted-foreground">
        <Glyph className="size-3.5" />
        {label}
      </p>
      <p className="mt-1 h-6 text-[20px] font-semibold leading-6 tabular-nums text-foreground">
        {value}
      </p>
      <div className="mt-0.5 h-4 truncate text-[11px] leading-4 tabular-nums text-muted-foreground">
        {detail}
      </div>
    </div>
  );
}

/** "↑ 12% vs prior 30 days"; "No data" with nothing earlier to compare. */
export function Change({ pct, against }: { pct: number | null; against: string }) {
  if (pct === null) return <>No data</>;
  const Arrow = pct > 0 ? ArrowUp : pct < 0 ? ArrowDown : null;
  return (
    <span className="flex min-w-0 items-center gap-1">
      <span
        className={cn(
          "flex shrink-0 items-center gap-0.5 font-medium",
          pct > 0 && "text-success",
          pct < 0 && "text-destructive",
        )}
      >
        {Arrow && <Arrow weight="bold" className="size-3" />}
        {Math.abs(pct)}%
      </span>
      <span className="truncate">vs {against}</span>
    </span>
  );
}
