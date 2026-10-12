import { cn } from "@/lib/utils";

/** A theme's colours side by side, background first. */
export function Swatch({ colors, className }: { colors: string[]; className?: string }) {
  return (
    <span
      aria-hidden
      className={cn("flex h-3.5 w-6 shrink-0 overflow-hidden rounded-[4px] ring-1 ring-border", className)}
    >
      {colors.length === 0 ? (
        <span className="flex-1 bg-secondary" />
      ) : (
        colors.map((c, i) => <span key={i} className="flex-1" style={{ backgroundColor: c }} />)
      )}
    </span>
  );
}
