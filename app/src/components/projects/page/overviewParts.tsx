import type { ReactNode } from "react";

/** Matches `Section`'s heading, for the parts that aren't row lists. */
export function Heading({ children }: { children: ReactNode }) {
  return (
    <h2 className="mb-2 px-0.5 text-[13px] font-semibold text-foreground">{children}</h2>
  );
}

/** `TaskPage`'s property row, but `items-start`: tags wrap and an event is
 *  two lines. */
export function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex min-h-8 items-start gap-3">
      <span className="w-24 shrink-0 pt-1.5 text-[11px] text-muted-foreground">{label}</span>
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-2 py-0.5">{children}</div>
    </div>
  );
}
