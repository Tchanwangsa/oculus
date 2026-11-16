import { useState, type MouseEvent, type ReactNode } from "react";
import { cn } from "@/lib/utils";

const ROW = "flex w-full min-w-0 items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left text-[12.5px] leading-5";

/** A card row: glyph, label, then muted trailing facts (pages, a status).
 *  With `onOpen` it is a button that opens its thing, ⌘-click or a middle
 *  click in a new tab, as a file chip does. */
export function InfoRow({
  icon,
  label,
  trailing,
  title,
  onOpen,
}: {
  icon: ReactNode;
  label: ReactNode;
  trailing?: ReactNode;
  title?: string;
  onOpen?: (newTab: boolean) => void;
}) {
  const body = (
    <>
      <span className="flex size-[15px] shrink-0 items-center justify-center">{icon}</span>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {trailing != null && trailing !== "" && (
        <span className="shrink-0 text-xs text-muted-foreground tabular-nums">{trailing}</span>
      )}
    </>
  );
  if (!onOpen) {
    return (
      <div data-info-row className={ROW} title={title}>
        {body}
      </div>
    );
  }
  return (
    <button
      type="button"
      data-info-row
      title={title}
      onClick={(e: MouseEvent) => onOpen(e.metaKey || e.ctrlKey)}
      onAuxClick={(e: MouseEvent) => {
        if (e.button !== 1) return;
        e.preventDefault();
        onOpen(true);
      }}
      className={cn(ROW, "cursor-pointer transition-colors hover:bg-accent focus-visible:bg-accent focus-visible:outline-none")}
    >
      {body}
    </button>
  );
}

/** How many rows a group shows before "Show N more". */
const LIMIT = 5;

/** A headed group of rows that ends in "Show N more" past `LIMIT`; the card
 *  grows to hold them and never scrolls inside itself. */
export function InfoGroup<T>({
  heading,
  rows,
  render,
}: {
  heading: string;
  rows: T[];
  render: (row: T) => ReactNode;
}) {
  const [all, setAll] = useState(false);
  if (rows.length === 0) return null;
  const hidden = rows.length - LIMIT;
  const shown = all || hidden <= 0 ? rows : rows.slice(0, LIMIT);
  return (
    <section aria-label={heading} className="flex flex-col border-t border-border-subtle py-1">
      <h3 className="px-2.5 pt-1.5 pb-0.5 font-sans text-[11px] font-medium text-muted-foreground">{heading}</h3>
      {shown.map(render)}
      {hidden > 0 && (
        <button
          type="button"
          onClick={() => setAll((a) => !a)}
          className="cursor-pointer rounded-lg px-2.5 py-1 text-left text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          {all ? "Show less" : `Show ${hidden} more`}
        </button>
      )}
    </section>
  );
}
