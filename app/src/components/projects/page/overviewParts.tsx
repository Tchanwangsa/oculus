import type { ReactNode } from "react";

/** Matches `Section`'s heading, for the parts that aren't row lists. */
export function Heading({ children }: { children: ReactNode }) {
  return (
    <h2 className="mb-2 px-0.5 text-[13px] font-semibold text-foreground">{children}</h2>
  );
}
