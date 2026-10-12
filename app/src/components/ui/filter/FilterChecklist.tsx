import { useMemo, useState } from "react";
import { MagnifyingGlass } from "@phosphor-icons/react";
import { Checkbox } from "@/components/ui/checkbox";
import { FilterRow } from "./FilterPopover";

export interface FilterOption {
  value: string;
  label: string;
  /** Rows it holds, drawn to the right. */
  count?: number;
}

/** Past this many options the checklist gets its own search field. */
const SEARCH_FROM = 8;

/**
 * A multi-select filter's body: one checkbox per option, picking never closes
 * the popover, and none ticked means no narrowing. Options counted 0 are
 * hidden; long lists get a search.
 */
export function FilterChecklist({
  options,
  selected,
  onChange,
  noun,
}: {
  options: readonly FilterOption[];
  selected: ReadonlySet<string>;
  onChange: (next: Set<string>) => void;
  /** Plural, for the search placeholder ("Search providers"). */
  noun: string;
}) {
  const [query, setQuery] = useState("");
  const q = query.trim().toLowerCase();
  // An option the other filters leave empty is dropped, unless it's ticked and
  // so still needs unticking.
  const live = useMemo(
    () => options.filter((o) => o.count !== 0 || selected.has(o.value)),
    [options, selected],
  );
  const shown = useMemo(
    () => (q ? live.filter((o) => o.label.toLowerCase().includes(q)) : live),
    [live, q],
  );

  const toggle = (value: string) => {
    const next = new Set(selected);
    if (next.has(value)) next.delete(value);
    else next.add(value);
    onChange(next);
  };

  return (
    <>
      {live.length > SEARCH_FROM && (
        <div className="mb-1 flex h-7 items-center gap-2 rounded-md border border-border-subtle px-2 focus-within:border-brand/50">
          <MagnifyingGlass size={12} className="shrink-0 text-muted-foreground" />
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={`Search ${noun}`}
            spellCheck={false}
            className="min-w-0 flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground"
          />
        </div>
      )}
      <div className="-mx-1 max-h-64 overflow-y-auto px-1">
        {shown.map((o) => (
          <label
            key={o.value}
            className="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-[12.5px] text-foreground transition-colors hover:bg-accent"
          >
            <Checkbox checked={selected.has(o.value)} onCheckedChange={() => toggle(o.value)} />
            <span className="min-w-0 flex-1 truncate">{o.label}</span>
            {o.count !== undefined && (
              <span className="shrink-0 text-[11px] text-muted-foreground tabular-nums">
                {o.count}
              </span>
            )}
          </label>
        ))}
        {shown.length === 0 && (
          <p className="px-2 py-3 text-center text-xs text-muted-foreground">No match.</p>
        )}
      </div>
      {selected.size > 0 && (
        <FilterRow
          label="Clear"
          selected={false}
          onPick={() => onChange(new Set())}
          className="mt-1 border-t border-border-subtle pt-1.5"
        />
      )}
    </>
  );
}
