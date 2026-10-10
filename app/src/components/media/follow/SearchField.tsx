import { MagnifyingGlass, X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

/** The search row over a `FollowList`. Escape clears and blurs; `count` shows
 *  whenever given, so the caller decides what counts as searching. */
export function SearchField({
  value,
  onChange,
  placeholder,
  count,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  /** Matches to show beside the clear button; `undefined` when not searching. */
  count?: number;
}) {
  const searching = count !== undefined;
  return (
    <div className="px-1.5 pt-1.5 shrink-0">
      <div className="relative">
        <MagnifyingGlass
          size={12}
          className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-muted-foreground"
        />
        <input
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              onChange("");
              e.currentTarget.blur();
            }
          }}
          placeholder={placeholder}
          spellCheck={false}
          className={cn(
            "w-full h-6 pl-6 rounded-full bg-surface text-[11px] text-foreground",
            "placeholder:text-muted-foreground focus:outline-none",
            "focus:ring-1 focus:ring-brand/40 transition-shadow",
            searching ? "pr-14" : "pr-2",
          )}
        />
        {searching && (
          <div className="absolute right-1 top-1/2 -translate-y-1/2 flex items-center gap-0.5">
            <span className="text-[10px] tabular-nums text-muted-foreground">
              {count}
            </span>
            <button
              onClick={() => onChange("")}
              aria-label="Clear search"
              className="p-0.5 rounded-full text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
            >
              <X size={10} weight="bold" />
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
