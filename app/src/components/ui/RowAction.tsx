import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

/**
 * A control inside the row's own `button`: a `div` with a button role, because
 * a nested `<button>` is invalid HTML and WebKit swallows its clicks. Stops
 * propagation so it never also triggers the row.
 */
export function RowAction({
  label,
  onTrigger,
  className,
  children,
}: {
  label: string;
  onTrigger: () => void;
  className?: string;
  children: ReactNode;
}) {
  return (
    <div
      role="button"
      tabIndex={0}
      data-tab-skip
      aria-label={label}
      title={label}
      onClick={(e) => {
        e.stopPropagation();
        onTrigger();
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          e.stopPropagation();
          onTrigger();
        }
      }}
      className={cn(
        "-m-1 p-1 rounded text-muted-foreground/50 transition-colors",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
        className,
      )}
    >
      {children}
    </div>
  );
}
