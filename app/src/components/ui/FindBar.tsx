import type { RefObject } from "react";
import { CaretDown, CaretUp, MagnifyingGlass, X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

const barButton =
  "flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground disabled:opacity-30 disabled:hover:bg-transparent transition-colors";

const field =
  "h-7 min-w-0 flex-1 bg-transparent text-[12.5px] text-foreground outline-none placeholder:text-muted-foreground";

export interface FindBarProps {
  inputRef: RefObject<HTMLInputElement | null>;
  query: string;
  onQueryChange: (query: string) => void;
  /** Enter, ⇧Enter and the arrow buttons; only offered with a query. */
  onStep: (backwards: boolean) => void;
  onClose: () => void;
  /** Shown beside the field: "No results", "3 of 12". */
  status?: string;
  placeholder: string;
  /** `row` sits under a toolbar; `floating` is a card at its positioned
   *  parent's top-right. */
  variant?: "row" | "floating";
  /** A row's border, which depends on what it sits against. */
  className?: string;
}

/**
 * The find bar (the in-app browser, the PDF viewer, a pane's DOM find). It
 * mounts focused; the owner selects the query on a repeat ⌘F through
 * `inputRef`. Escape closes without reaching the window's own Escape
 * handlers. `data-find-skip` keeps it out of the DOM find it drives.
 */
export function FindBar({
  inputRef,
  query,
  onQueryChange,
  onStep,
  onClose,
  status,
  placeholder,
  variant = "row",
  className,
}: FindBarProps) {
  return (
    <div
      data-find-skip
      className={cn(
        variant === "floating"
          ? "absolute right-3 top-3 z-40 w-[min(26rem,calc(100%-1.5rem))] rounded-xl border border-border bg-popover px-1.5 shadow-lg"
          : "px-2",
        className,
      )}
    >
      <div className="flex h-9 items-center gap-1">
        <MagnifyingGlass size={13} className="mx-1.5 shrink-0 text-muted-foreground" />
        <input
          ref={inputRef}
          autoFocus
          value={query}
          onChange={(e) => onQueryChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              onStep(e.shiftKey);
            }
            if (e.key === "Escape") {
              e.stopPropagation();
              onClose();
            }
          }}
          spellCheck={false}
          placeholder={placeholder}
          className={field}
        />
        {query && status && (
          <span className="mr-1 shrink-0 text-[11.5px] tabular-nums text-muted-foreground">
            {status}
          </span>
        )}
        <button
          onClick={() => onStep(true)}
          disabled={!query}
          aria-label="Previous match"
          className={barButton}
        >
          <CaretUp size={13} />
        </button>
        <button
          onClick={() => onStep(false)}
          disabled={!query}
          aria-label="Next match"
          className={barButton}
        >
          <CaretDown size={13} />
        </button>
        <button onClick={onClose} aria-label="Close find bar" className={barButton}>
          <X size={13} />
        </button>
      </div>
    </div>
  );
}
