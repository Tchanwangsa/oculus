import { useEffect, useMemo, useState } from "react";
import { ArrowLeft, ArrowRight } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

/**
 * The footer of a full-bleed table: page controls on the left, the row total
 * on the right, sitting on a top hairline. It is a footer, not a floating
 * control — it spans the table's full width and stays put while the body
 * scrolls under it.
 */
export function TablePagination({
  page,
  pageCount,
  onPage,
  total,
  unit,
  className,
}: {
  page: number;
  pageCount: number;
  onPage: (page: number) => void;
  /** Rows across every page — what the right-hand total counts. */
  total: number;
  /** Singular noun for the total, pluralised with a trailing "s". */
  unit: string;
  className?: string;
}) {
  // The box is free text while it is being typed in; it only commits on Enter
  // or blur, so a half-typed "1" of "12" never jumps the table to page 1.
  const [draft, setDraft] = useState(String(page));
  useEffect(() => setDraft(String(page)), [page]);

  const commit = () => {
    const n = Number(draft);
    if (Number.isFinite(n)) onPage(Math.min(pageCount, Math.max(1, Math.trunc(n))));
    else setDraft(String(page));
  };

  return (
    <div
      className={cn(
        "shrink-0 flex items-center gap-3 border-t border-border-subtle px-5 py-2.5",
        className,
      )}
    >
      <Button
        variant="ghost"
        size="icon-xs"
        aria-label="Previous page"
        disabled={page <= 1}
        onClick={() => onPage(page - 1)}
        className="text-muted-foreground hover:text-foreground"
      >
        <ArrowLeft size={13} />
      </Button>

      <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
        <span>Page</span>
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value.replace(/[^0-9]/g, ""))}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
            if (e.key === "Escape") setDraft(String(page));
          }}
          aria-label="Page number"
          className="h-6 w-9 rounded-md border border-border-subtle bg-surface/60 text-center text-[11px] tabular-nums text-foreground outline-none transition-colors focus:border-brand/50 focus:bg-card"
        />
        <span className="tabular-nums">of {pageCount}</span>
      </div>

      <Button
        variant="ghost"
        size="icon-xs"
        aria-label="Next page"
        disabled={page >= pageCount}
        onClick={() => onPage(page + 1)}
        className="text-muted-foreground hover:text-foreground"
      >
        <ArrowRight size={13} />
      </Button>

      <span className="ml-auto text-[11px] text-muted-foreground tabular-nums">
        {total} {unit}
        {total === 1 ? "" : "s"}
      </span>
    </div>
  );
}

/** Page state for a list, clamped so deleting rows can't strand the view on a
 *  page that no longer exists. */
export function usePagedRows<T>(rows: T[], pageSize: number) {
  const [page, setPage] = useState(1);
  const pageCount = Math.max(1, Math.ceil(rows.length / pageSize));
  const clamped = Math.min(page, pageCount);

  useEffect(() => {
    if (page > pageCount) setPage(pageCount);
  }, [page, pageCount]);

  const pageRows = useMemo(
    () => rows.slice((clamped - 1) * pageSize, clamped * pageSize),
    [rows, clamped, pageSize],
  );

  return { page: clamped, pageCount, setPage, pageRows };
}
