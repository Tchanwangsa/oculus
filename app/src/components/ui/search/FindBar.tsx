import { useRef, type RefObject } from "react";
import { CaretDown, CaretRight, CaretUp, MagnifyingGlass, X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

const barButton =
  "flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground disabled:opacity-30 disabled:hover:bg-transparent transition-colors";

const textButton =
  "flex h-6 shrink-0 items-center rounded-full px-2.5 text-[11.5px] text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground disabled:opacity-30 disabled:hover:bg-transparent transition-colors";

const field =
  "h-7 min-w-0 flex-1 bg-transparent text-[12.5px] text-foreground outline-none placeholder:text-muted-foreground";

/** The optional second row; the owner holds its text and whether it shows. */
export interface FindReplace {
  value: string;
  onChange: (value: string) => void;
  /** Enter in the field, or the Replace button: the current match. */
  onReplace: () => void;
  /** ⌘Enter / ⌥Enter in the field, or the All button: every match. */
  onReplaceAll: () => void;
  open: boolean;
  onToggle: () => void;
}

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
  /** Adds a chevron that unfolds a replace row. */
  replace?: FindReplace;
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
  replace,
  className,
}: FindBarProps) {
  const replaceRef = useRef<HTMLInputElement>(null);
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
        {replace ? (
          <button
            onClick={() => {
              if (!replace.open) requestAnimationFrame(() => replaceRef.current?.focus());
              replace.onToggle();
            }}
            aria-label={replace.open ? "Hide replace" : "Show replace"}
            aria-expanded={replace.open}
            className={barButton}
          >
            <CaretRight
              size={13}
              className={cn("transition-transform", replace.open && "rotate-90")}
            />
          </button>
        ) : (
          <MagnifyingGlass size={13} className="mx-1.5 shrink-0 text-muted-foreground" />
        )}
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
      {replace?.open && (
        <div className="flex h-9 items-center gap-1">
          {/* Lines the field up under the find field. */}
          <span aria-hidden className="w-7 shrink-0" />
          <input
            ref={replaceRef}
            value={replace.value}
            onChange={(e) => replace.onChange(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                if (!query) return;
                if (e.metaKey || e.altKey) replace.onReplaceAll();
                else replace.onReplace();
              }
              if (e.key === "Escape") {
                e.stopPropagation();
                onClose();
              }
            }}
            spellCheck={false}
            placeholder="Replace"
            className={field}
          />
          <button onClick={replace.onReplace} disabled={!query} className={textButton}>
            Replace
          </button>
          <button
            onClick={replace.onReplaceAll}
            disabled={!query}
            aria-label="Replace all"
            className={textButton}
          >
            All
          </button>
        </div>
      )}
    </div>
  );
}
