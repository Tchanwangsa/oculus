import { useEffect, useRef, type ReactNode } from "react";

import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { cn } from "@/lib/utils";

/**
 * A quieter tab strip for views subordinate to a `ViewTabs` above them (e.g.
 * Board / Table under Tasks): smaller pills, the active one a `secondary` fill,
 * so two stacked strips don't read as the same rank. With `scroll` the strip
 * keeps to one line and scrolls sideways when it overflows, its edges faded
 * (`useScrollFade`) and the chosen pill kept in view.
 */
export function PillTabs<T extends string>({
  tabs,
  value,
  onChange,
  className,
  scroll,
}: {
  tabs: ReadonlyArray<{ value: T; label: ReactNode }>;
  value: T;
  onChange: (value: T) => void;
  className?: string;
  scroll?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useScrollFade(ref, "x");
  useEffect(() => {
    const el = ref.current;
    const pill = el?.querySelector<HTMLElement>('[aria-selected="true"]');
    if (!el || !pill) return;
    // Only this strip moves; scrollIntoView could scroll the page too.
    if (pill.offsetLeft < el.scrollLeft) el.scrollLeft = pill.offsetLeft;
    else if (pill.offsetLeft + pill.offsetWidth > el.scrollLeft + el.clientWidth) {
      el.scrollLeft = pill.offsetLeft + pill.offsetWidth - el.clientWidth;
    }
  }, [value]);
  const list = (
    <div role="tablist" className={cn("flex items-center gap-0.5", scroll && "w-max", className)}>
      {tabs.map((tab) => {
        const active = tab.value === value;
        return (
          <button
            key={tab.value}
            type="button"
            role="tab"
            aria-selected={active}
            onClick={() => onChange(tab.value)}
            className={cn(
              "shrink-0 cursor-pointer whitespace-nowrap rounded-full px-2 py-0.5 text-[11.5px] font-medium transition-colors",
              active
                ? "bg-secondary text-foreground"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            {tab.label}
          </button>
        );
      })}
    </div>
  );
  if (!scroll) return list;
  return (
    <div ref={ref} className="relative min-w-0 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
      {list}
    </div>
  );
}
