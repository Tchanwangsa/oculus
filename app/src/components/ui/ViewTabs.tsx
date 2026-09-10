import { cn } from "@/lib/utils";

export interface ViewTab<T extends string> {
  value: T;
  label: string;
  /** Optional trailing element — a count chip, a dot. */
  badge?: React.ReactNode;
}

/**
 * A non-routed underline tab strip: the same shape as `SubjectLayout`'s nav,
 * for switching what a page renders rather than where it navigates. Inactive
 * tabs are greyed out and only the active one carries the indigo rule, so a
 * page's views read as siblings instead of hiding inside a dropdown.
 *
 * The strip is meant to sit on a container's bottom border — `-mb-px` pulls
 * the active underline down onto that line rather than floating above it.
 */
export function ViewTabs<T extends string>({
  tabs,
  value,
  onChange,
  className,
}: {
  tabs: ReadonlyArray<ViewTab<T>>;
  value: T;
  onChange: (value: T) => void;
  className?: string;
}) {
  return (
    <div role="tablist" className={cn("flex items-center gap-4", className)}>
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
              "-mb-px flex cursor-pointer items-center gap-1.5 border-b-2 pb-2.5 pt-1 text-[13px] font-medium transition-colors",
              active
                ? "border-primary text-foreground"
                : "border-transparent text-muted-foreground hover:text-foreground",
            )}
          >
            {tab.label}
            {tab.badge}
          </button>
        );
      })}
    </div>
  );
}
