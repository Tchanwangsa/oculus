import type { ComponentProps, ReactNode } from "react";
import { cn } from "@/lib/utils";
import { TablePagination } from "@/components/ui/table/TablePagination";

/**
 * The shell every full-bleed table shares (`SyncHistoryTable` is the model):
 * one `cols` grid class for the header and every row, the header outside the
 * scroller, and a pagination footer.
 *
 * Column alignment rests on the 6px classic scrollbar (`index.css`): the
 * header's `pr-1.5` stands in for it, and the body is `overflow-y: scroll` so
 * the gutter is there even while the rows fit (`scrollbar-gutter` is a no-op
 * in WebKit — see `.page-scroll`).
 */
export function GridTable({
  cols,
  header,
  empty,
  pagination,
  children,
}: {
  /** The `grid grid-cols-[…] … px-5` class every row also uses. */
  cols: string;
  header: ReactNode;
  /** Shown above the rows when set — pass it only while the table is empty. */
  empty?: ReactNode;
  pagination: ComponentProps<typeof TablePagination>;
  children: ReactNode;
}) {
  return (
    <div className="flex h-full flex-col">
      <div className="shrink-0 pr-1.5">
        <div className={cn(cols, "border-b border-border-subtle bg-card py-2")}>{header}</div>
      </div>

      <div className="flex-1 min-h-0 overflow-y-scroll">
        {empty && (
          <p className="px-5 py-16 text-center text-xs text-muted-foreground">{empty}</p>
        )}
        {children}
      </div>

      <TablePagination {...pagination} />
    </div>
  );
}

/** Plain column labels; `endLast` right-aligns the last, over a status badge. */
export function HeaderLabels({
  labels,
  endLast = false,
}: {
  labels: readonly string[];
  endLast?: boolean;
}) {
  return labels.map((h, i) => (
    <span
      key={i}
      className={cn(
        "text-[11px] font-medium text-muted-foreground",
        endLast && i === labels.length - 1 && "justify-self-end",
      )}
    >
      {h}
    </span>
  ));
}
