import { useRef, type ReactNode } from "react";
import { cn } from "@/lib/utils";
import { useScrollFade } from "@/hooks/ui/useScrollFade";

/**
 * Toggle chips on one line that scrolls sideways with faded edges instead of
 * wrapping. It runs out to the edges of the `px-4` popover it sits in
 * (`-mx-4`), so chips slide under the fade rather than stopping short of it;
 * the scrollbar is hidden.
 */
export function ChipRow({ children }: { children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useScrollFade(ref, "x");
  return (
    <div
      ref={ref}
      className="-mx-4 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
    >
      <div className="flex w-max items-center gap-1.5 px-4">{children}</div>
    </div>
  );
}

/** A pill that toggles on and off, brand-tinted while on. */
export function ToggleChip({
  selected,
  onClick,
  children,
}: {
  selected: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-pressed={selected}
      onClick={onClick}
      className={cn(
        "inline-flex h-7 shrink-0 cursor-pointer items-center gap-1.5 rounded-full border px-2.5 text-xs whitespace-nowrap outline-none transition-[color,background-color,border-color]",
        "focus-visible:ring-[3px] focus-visible:ring-ring/50",
        selected
          ? "border-brand/40 bg-brand-muted text-brand"
          : "border-border text-muted-foreground hover:bg-accent hover:text-foreground",
      )}
    >
      {children}
    </button>
  );
}
