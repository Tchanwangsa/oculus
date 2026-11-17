import type { ReactNode } from "react";
import { MagnifyingGlass, X } from "@phosphor-icons/react";
import { Breadcrumb, type BreadcrumbItem } from "@/components/ui/Breadcrumb";

/**
 * The browser's 48px header: the trail on the left; the search, scoped to
 * the folder on screen and everything below it, and the New menu (`action`)
 * on the right.
 */
export function DriveHeader({ crumbs, leading, query, onQuery, placeholder, action }: {
  crumbs: BreadcrumbItem[];
  /** Before the trail: Drive's nav toggle, when the page has one. */
  leading?: ReactNode;
  query: string;
  onQuery: (query: string) => void;
  placeholder: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex h-12 shrink-0 items-center gap-3 px-5">
      {leading}
      <Breadcrumb items={crumbs} />
      <div className="flex h-8 w-52 min-w-28 shrink items-center gap-2 rounded-full border border-input bg-card px-3 transition-[color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25">
        <MagnifyingGlass size={13} className="shrink-0 text-muted-foreground" />
        <input
          value={query}
          onChange={(e) => onQuery(e.target.value)}
          onKeyDown={(e) => {
            e.stopPropagation();
            if (e.key === "Escape" && query) {
              e.preventDefault();
              onQuery("");
            }
          }}
          placeholder={placeholder}
          aria-label={placeholder}
          spellCheck={false}
          autoComplete="off"
          className="min-w-0 flex-1 bg-transparent text-[13px] text-foreground outline-none placeholder:text-muted-foreground"
        />
        {query && (
          <button
            type="button"
            aria-label="Clear search"
            onClick={() => onQuery("")}
            className="-mr-1 flex size-5 shrink-0 cursor-pointer items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          >
            <X size={11} weight="bold" />
          </button>
        )}
      </div>
      {action}
    </div>
  );
}
