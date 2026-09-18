import { cn } from "@/lib/utils";

/**
 * A quiet tab strip, a step below `ViewTabs` in every dimension it can be:
 * smaller, no underline, and the active one marked by a fill rather than a
 * colour.
 *
 * It exists because two identical underline strips stacked read as two levels
 * of the same rank and fight each other for the indigo rule. So where a page
 * already has a `ViewTabs` above it and needs a *subordinate* set of
 * views — Board / Table / Timeline under a project's Tasks tab, Board / Table
 * under the universal Tasks page's scope — this is the second strip.
 *
 * Pills, per the root `CLAUDE.md`: `rounded-full`, and the fill is `secondary`
 * rather than `primary`, which is reserved for the thing a page is actually
 * doing.
 */
export function PillTabs<T extends string>({
  tabs,
  value,
  onChange,
  className,
}: {
  tabs: ReadonlyArray<{ value: T; label: string }>;
  value: T;
  onChange: (value: T) => void;
  className?: string;
}) {
  return (
    <div role="tablist" className={cn("flex items-center gap-0.5", className)}>
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
              "cursor-pointer rounded-full px-2 py-0.5 text-[11.5px] font-medium transition-colors",
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
}
