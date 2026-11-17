import type { ReactNode } from "react";
import { List, SquaresFour } from "@phosphor-icons/react";
import { FilterChecklist } from "@/components/ui/filter/FilterChecklist";
import { FilterChips, type FilterChip } from "@/components/ui/filter/FilterChips";
import { FilterPopover, FilterRow } from "@/components/ui/filter/FilterPopover";
import {
  KIND_LABEL,
  KINDS,
  MODIFIED_LABEL,
  MODIFIED_RANGES,
  NO_FILTERS,
  SOURCE_LABEL,
  SOURCES,
  type DriveFilters,
  type DriveKind,
  type DriveSource,
} from "@/lib/drive/filters";
import { cn } from "@/lib/utils";

export type DriveLayout = "grid" | "list";

const pillLabel = <T extends string>(noun: string, picked: ReadonlySet<T>, labels: Record<T, string>) =>
  picked.size === 0 ? noun : picked.size === 1 ? labels[[...picked][0]] : `${noun} · ${picked.size}`;

/**
 * Type, From and Modified pills (any of them turns the folder into a flat
 * list of everything below it), the grid/list switch, and under them the
 * applied filters as chips. A fixed 40px row, so switching never moves the
 * entries.
 */
export function DriveToolbar({ filters, onFilters, layout, onLayout }: {
  filters: DriveFilters;
  onFilters: (next: DriveFilters) => void;
  layout: DriveLayout;
  onLayout: (layout: DriveLayout) => void;
}) {
  const set = (patch: Partial<DriveFilters>) => onFilters({ ...filters, ...patch });
  const chips: FilterChip[] = [
    ...[...filters.kinds].map((k) => ({
      key: `kind:${k}`,
      label: KIND_LABEL[k],
      onRemove: () => set({ kinds: without(filters.kinds, k) }),
    })),
    ...[...filters.sources].map((s) => ({
      key: `from:${s}`,
      label: SOURCE_LABEL[s],
      onRemove: () => set({ sources: without(filters.sources, s) }),
    })),
    ...(filters.modified === "any"
      ? []
      : [{ key: "modified", label: MODIFIED_LABEL[filters.modified], onRemove: () => set({ modified: "any" }) }]),
  ];

  return (
    <>
      <div className="flex h-10 shrink-0 items-center gap-1.5 px-5">
        <FilterPopover label={pillLabel("Type", filters.kinds, KIND_LABEL)} active={filters.kinds.size > 0}>
          {() => (
            <FilterChecklist
              noun="types"
              options={KINDS.map((k) => ({ value: k, label: KIND_LABEL[k] }))}
              selected={filters.kinds}
              onChange={(next) => set({ kinds: next as Set<DriveKind> })}
            />
          )}
        </FilterPopover>
        <FilterPopover label={pillLabel("From", filters.sources, SOURCE_LABEL)} active={filters.sources.size > 0}>
          {() => (
            <FilterChecklist
              noun="sources"
              options={SOURCES.map((s) => ({ value: s, label: SOURCE_LABEL[s] }))}
              selected={filters.sources}
              onChange={(next) => set({ sources: next as Set<DriveSource> })}
            />
          )}
        </FilterPopover>
        <FilterPopover
          label={filters.modified === "any" ? "Modified" : MODIFIED_LABEL[filters.modified]}
          active={filters.modified !== "any"}
        >
          {(close) =>
            MODIFIED_RANGES.map((range) => (
              <FilterRow
                key={range}
                label={MODIFIED_LABEL[range]}
                selected={filters.modified === range}
                onPick={() => { set({ modified: range }); close(); }}
              />
            ))
          }
        </FilterPopover>
        <span className="flex-1" />
        <div role="radiogroup" aria-label="View" className="flex shrink-0 items-center rounded-lg border border-border-subtle p-0.5">
          <LayoutButton label="Grid" active={layout === "grid"} onClick={() => onLayout("grid")}>
            <SquaresFour size={14} />
          </LayoutButton>
          <LayoutButton label="List" active={layout === "list"} onClick={() => onLayout("list")}>
            <List size={14} />
          </LayoutButton>
        </div>
      </div>
      <FilterChips chips={chips} onClear={() => onFilters({ ...NO_FILTERS, query: filters.query })} />
    </>
  );
}

function without<T>(set: ReadonlySet<T>, value: T): Set<T> {
  const next = new Set(set);
  next.delete(value);
  return next;
}

function LayoutButton({ label, active, onClick, children }: {
  label: string;
  active: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={active}
      aria-label={label}
      title={label}
      onClick={onClick}
      className={cn(
        "flex h-6 w-7 cursor-pointer items-center justify-center rounded-md transition-colors",
        active ? "bg-accent text-foreground" : "text-muted-foreground hover:text-foreground",
      )}
    >
      {children}
    </button>
  );
}
