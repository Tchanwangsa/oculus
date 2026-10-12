import { cn } from "@/lib/utils";

/** One choice of a few, as pills in a row. */
export function Segmented<T extends string | number>({
  label,
  options,
  value,
  onChange,
  dense = false,
}: {
  label: string;
  options: ReadonlyArray<{ value: T; label: string }>;
  value: T;
  onChange: (value: T) => void;
  /** Tighter pills, for a row of six. */
  dense?: boolean;
}) {
  return (
    <div role="radiogroup" aria-label={label} className="flex items-center gap-0.5 rounded-full bg-secondary p-0.5">
      {options.map((o) => {
        const on = o.value === value;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={on}
            onClick={() => onChange(o.value)}
            className={cn(
              "flex-1 cursor-pointer whitespace-nowrap rounded-full py-1 text-[11.5px] font-medium transition-colors",
              dense ? "px-1.5" : "px-2.5",
              on ? "bg-card text-foreground shadow-xs" : "text-muted-foreground hover:text-foreground",
            )}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}
