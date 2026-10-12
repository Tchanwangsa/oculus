import { MagnifyingGlass, X } from "@phosphor-icons/react";
import { FilterPopover } from "@/components/ui/filter/FilterPopover";
import { FilterChecklist, type FilterOption } from "@/components/ui/filter/FilterChecklist";
import { FilterChips } from "@/components/ui/filter/FilterChips";
import { THREAD_STATUSES, THREAD_TYPES, threadStatus, type EdThread, type ThreadFilters as Filters } from "@/lib/ed/threads";
import { CategoryDot, ThreadKindIcon, ThreadStatusIcon } from "./ThreadIcons";

function countedOptions(options: readonly FilterOption[], values: readonly (string | null)[]): FilterOption[] {
  const counts = new Map<string, number>();
  for (const value of values) if (value != null) counts.set(value, (counts.get(value) ?? 0) + 1);
  return options.map((option) => ({ ...option, count: counts.get(option.value) ?? 0 }));
}

export function ThreadFilters({ threads, filters, onChange, shown, loading }: {
  threads: EdThread[];
  filters: Filters;
  onChange: (filters: Filters) => void;
  shown: number;
  loading: boolean;
}) {
  // A category's colour is its top-level category's, so any of its threads has it.
  const colours = new Map<string, string | null>();
  for (const thread of threads) {
    const key = thread.category ?? "Uncategorised";
    if (!colours.get(key)) colours.set(key, thread.categoryColor);
  }
  const categories = [...colours.keys()].sort().map((value) => ({
    value, label: value, icon: <CategoryDot color={colours.get(value) ?? null} />,
  }));
  const types = THREAD_TYPES.map((option) => ({
    ...option, icon: <ThreadKindIcon kind={option.value} size={13} />,
  }));
  const statuses = THREAD_STATUSES.map((option) => ({
    ...option, icon: <ThreadStatusIcon status={option.value} size={13} />,
  }));
  const groups = [
    { key: "types" as const, label: "Type", noun: "types", options: countedOptions(types, threads.map((t) => t.kind ?? "unknown")) },
    { key: "statuses" as const, label: "Status", noun: "statuses", options: countedOptions(statuses, threads.map(threadStatus)) },
    { key: "categories" as const, label: "Category", noun: "categories", options: countedOptions(categories, threads.map((t) => t.category ?? "Uncategorised")) },
  ];
  const clear = () => onChange({ ...filters, types: new Set(), statuses: new Set(), categories: new Set() });
  const chips = groups.flatMap(({ key, options }) => [...filters[key]].map((value) => ({
    key: `${key}:${value}`,
    label: options.find((option) => option.value === value)?.label ?? value,
    onRemove: () => { const next = new Set(filters[key]); next.delete(value); onChange({ ...filters, [key]: next }); },
  })));
  return (
    <div className="shrink-0 pt-5">
      <div className="flex flex-wrap items-center gap-3 px-6 pb-4">
        <div className="flex h-10 min-w-48 flex-1 items-center gap-2.5 rounded-full border border-border-subtle bg-surface px-3.5 focus-within:border-brand/50 focus-within:ring-2 focus-within:ring-brand/10">
          <MagnifyingGlass size={17} className="shrink-0 text-muted-foreground" />
          <input
            type="search"
            aria-label="Search Ed titles, posts and replies"
            placeholder="Search titles, posts and replies…"
            value={filters.query}
            onChange={(event) => onChange({ ...filters, query: event.target.value })}
            spellCheck={false}
            className="min-w-0 flex-1 bg-transparent text-[13px] outline-none placeholder:text-muted-foreground [&::-webkit-search-cancel-button]:hidden"
          />
          {filters.query && <button type="button" aria-label="Clear search" onClick={() => onChange({ ...filters, query: "" })} className="rounded-full p-1 text-muted-foreground hover:bg-accent hover:text-foreground"><X size={14} /></button>}
        </div>
        <span role="status" className="shrink-0 text-xs text-muted-foreground tabular-nums">
          {loading ? "Loading threads…" : `${shown === threads.length ? shown : `${shown} of ${threads.length}`} ${threads.length === 1 ? "thread" : "threads"}`}
        </span>
      </div>
      <div className="flex flex-wrap items-center gap-2 px-6 pb-3">
        {groups.map(({ key, label, noun, options }) => (
          <FilterPopover key={key} label={`${label}${filters[key].size ? ` (${filters[key].size})` : ""}`} active={filters[key].size > 0}>
            {() => <FilterChecklist options={options} selected={filters[key]} onChange={(next) => onChange({ ...filters, [key]: next })} noun={noun} />}
          </FilterPopover>
        ))}
      </div>
      <FilterChips chips={chips} onClear={clear} />
    </div>
  );
}
