import type { ReactNode } from "react";
import { CaretDown, CaretUp, MagnifyingGlass, X } from "@phosphor-icons/react";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

/** The toolbar's search field, with a clear button once there is a query. */
export function SearchField({
  value,
  onChange,
  placeholder,
  disabled,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  disabled?: boolean;
}) {
  return (
    <div className="flex h-7 w-56 shrink-0 items-center gap-2 rounded-full border border-border bg-card px-2.5 focus-within:border-brand/50">
      <MagnifyingGlass size={13} className="shrink-0 text-muted-foreground" />
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        disabled={disabled}
        spellCheck={false}
        className="min-w-0 flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground disabled:opacity-50"
      />
      {value && (
        <button
          type="button"
          aria-label="Clear search"
          onClick={() => onChange("")}
          className="shrink-0 cursor-pointer text-muted-foreground hover:text-foreground"
        >
          <X size={11} />
        </button>
      )}
    </div>
  );
}

/** A ghost icon button whose tooltip is its accessible name. */
export function IconAction({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button variant="ghost" size="icon-xs" aria-label={label} disabled={disabled} onClick={onClick}>
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

/** An empty table with its way out (clear the query, widen the scope). */
export function TableEmpty({ message, children }: { message: ReactNode; children?: ReactNode }) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 px-5 py-16 text-center">
      <p className="max-w-sm text-xs text-muted-foreground">{message}</p>
      {children}
    </div>
  );
}

export type SortDir = "asc" | "desc";

/** A column label that sorts on click; the active one carries a caret. */
export function SortHeader<K extends string>({
  label,
  column,
  sort,
  onSort,
  end = false,
}: {
  label: string;
  column: K;
  sort: { key: K; dir: SortDir };
  onSort: (key: K) => void;
  /** Right-aligned, over a number column. */
  end?: boolean;
}) {
  const active = sort.key === column;
  const Caret = sort.dir === "asc" ? CaretUp : CaretDown;
  return (
    <button
      type="button"
      onClick={() => onSort(column)}
      aria-sort={active ? (sort.dir === "asc" ? "ascending" : "descending") : undefined}
      className={cn(
        "flex min-w-0 cursor-pointer items-center gap-1 text-[11px] font-medium transition-colors",
        active ? "text-foreground" : "text-muted-foreground hover:text-foreground",
        end && "justify-self-end",
      )}
    >
      <span className="truncate">{label}</span>
      {active && <Caret size={9} weight="bold" className="shrink-0" />}
    </button>
  );
}
