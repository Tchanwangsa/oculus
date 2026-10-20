import { CaretDown, CaretUp } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { sortByHeader } from "../display/settings";
import type { TaskSort } from "../tasks/taskSort";
import { COLUMNS, type ColumnId } from "./tasksColumns";

/** Sortable column labels shared by the project index and task table. */
export function TaskTableHeaders({ columns, sort: selectedSort, onSort }: {
  columns: readonly ColumnId[];
  sort: TaskSort;
  onSort: (sort: TaskSort) => void;
}) {
  return <>{columns.map((id) => {
        const { label, sort } = COLUMNS[id];
        if (sort == null) {
          return (
            <span key={id} className="truncate text-[11px] font-medium text-muted-foreground">
              {label}
            </span>
          );
        }
        const current = selectedSort.key === sort;
        return (
          <button
            key={id}
            type="button"
            aria-label={`Sort by ${label}`}
            onClick={() => onSort(sortByHeader(selectedSort, sort))}
            className={cn(
              "flex min-w-0 cursor-pointer items-center gap-1 text-left text-[11px] font-medium transition-colors",
              current ? "text-foreground" : "text-muted-foreground hover:text-foreground",
            )}
          >
            <span className="truncate">{label}</span>
            {current &&
              (selectedSort.dir === "asc" ? (
                <CaretUp size={9} weight="bold" className="shrink-0" />
              ) : (
                <CaretDown size={9} weight="bold" className="shrink-0" />
              ))}
          </button>
        );
      })}</>;
}
