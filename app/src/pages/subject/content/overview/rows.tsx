import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

export function SectionHeading({ children, aside }: { children: ReactNode; aside?: ReactNode }) {
  return (
    <h2 className="mb-2 flex min-w-0 items-baseline gap-2 px-0.5 text-[13px] font-semibold text-foreground">
      <span className="truncate">{children}</span>
      {aside != null && (
        <span className="shrink-0 text-[11.5px] font-normal text-muted-foreground tabular-nums">{aside}</span>
      )}
    </h2>
  );
}

/** A section with nothing in it: one muted line, not a card. */
export function Quiet({ children }: { children: ReactNode }) {
  return <p className="px-0.5 py-1 text-[12px] text-muted-foreground">{children}</p>;
}

interface RowProps {
  children: ReactNode;
  onOpen?: () => void;
  tabHref?: string;
  title?: string;
  muted?: boolean;
}

/** A `ListCard` row: the subject tabs' row grammar. Without `onOpen` it is plain text. */
export function ListRow({ children, onOpen, tabHref, title, muted }: RowProps) {
  const className = cn(
    "flex w-full min-w-0 items-center gap-3 px-3 py-2.5 text-left text-[12px] text-foreground",
    muted && "opacity-60",
  );
  if (!onOpen) return <div title={title} className={className}>{children}</div>;
  return (
    <button
      type="button"
      data-tab-href={tabHref}
      title={title}
      onClick={onOpen}
      className={cn(className, "transition-colors hover:bg-surface")}
    >
      {children}
    </button>
  );
}

/** A side-column row: smaller, undivided. */
export function MiniRow({ children, onOpen, tabHref, title, muted }: RowProps) {
  const className = cn(
    "-mx-1.5 flex w-[calc(100%+0.75rem)] min-w-0 items-center gap-2 rounded-md px-1.5 py-1 text-left text-[12px]",
    muted ? "text-muted-foreground/70" : "text-foreground",
  );
  if (!onOpen) return <div title={title} className={className}>{children}</div>;
  return (
    <button
      type="button"
      data-tab-href={tabHref}
      title={title}
      onClick={onOpen}
      className={cn(className, "transition-colors hover:bg-surface")}
    >
      {children}
    </button>
  );
}

export function RowTitle({ children }: { children: ReactNode }) {
  return <span className="min-w-0 flex-1 truncate">{children}</span>;
}

/** The right-hand date of a row. */
export function RowDate({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <span className={cn("shrink-0 text-[11px] text-muted-foreground tabular-nums", className)}>{children}</span>
  );
}
