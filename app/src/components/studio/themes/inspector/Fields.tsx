import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

/** `accentStrong` -> `Accent strong`, `h1` -> `H1`: a token name as a label. */
export function tokenLabel(name: string): string {
  const spaced = name.replace(/([a-z])([A-Z])/g, "$1 $2").toLowerCase();
  return spaced.charAt(0).toUpperCase() + spaced.slice(1);
}

/** Fields in equal columns: two for a run of numbers, more where a value is
 *  a few small numbers (an easing's four points). */
export function FieldGrid({ children, cols = 2, className }: { children: ReactNode; cols?: 2 | 3 | 4; className?: string }) {
  return (
    <div
      className={cn(
        "grid gap-x-2 gap-y-2.5",
        cols === 2 && "grid-cols-2",
        cols === 3 && "grid-cols-3",
        cols === 4 && "grid-cols-4",
        className,
      )}
    >
      {children}
    </div>
  );
}

/** What a group of tokens does, in a sentence under them. */
export function Note({ children }: { children: ReactNode }) {
  return <p className="pt-2 text-[11.5px] leading-snug text-muted-foreground">{children}</p>;
}
