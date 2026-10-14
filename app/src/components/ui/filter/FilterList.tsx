import type { ReactNode } from "react";

/**
 * The scroll region of a long single-choice list. `overflow-y: scroll` keeps
 * the scrollbar's gutter whether or not the rows overflow — WebKit ignores
 * `scrollbar-gutter` — so the popover's width never shifts, and the height is
 * capped by a fixed `max-h` (also by the room the popover has).
 */
export function FilterList({ children }: { children: ReactNode }) {
  return (
    <div className="max-h-[min(16rem,calc(var(--radix-popover-content-available-height)-1rem))] overflow-y-scroll">
      {children}
    </div>
  );
}
