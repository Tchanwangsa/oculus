import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

/** Two faint indigo rings around a bookend's 64px hero art: squircles for
 *  the Oculus mark on Welcome, circles (`round`) for Done's check. */
export function HeroHalo({ children, round = false }: { children: ReactNode; round?: boolean }) {
  return (
    <div className="relative mt-5 mb-4 flex size-16 items-center justify-center">
      <span
        aria-hidden
        className={cn(
          "absolute -inset-7 border border-brand/10 bg-brand-muted/25",
          round ? "rounded-full" : "rounded-[40px]",
        )}
      />
      <span
        aria-hidden
        className={cn(
          "absolute -inset-3.5 border border-brand/15 bg-brand-muted/50",
          round ? "rounded-full" : "rounded-[28px]",
        )}
      />
      <div className="relative">{children}</div>
    </div>
  );
}
