import type { ReactNode } from "react";

/** A responsive grid of `MediaTile`s: as many 240px-or-wider columns as fit. */
export function MediaGrid({ children }: { children: ReactNode }) {
  return <div className="grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-x-4 gap-y-6">{children}</div>;
}

/**
 * A 16:9 picture over a title and a line of facts. The whole tile is one
 * button, so anything on it (a badge, a bar) is a plain element, never a
 * control of its own.
 */
export function MediaTile({
  picture,
  badge,
  progress,
  title,
  meta,
  href,
  onOpen,
}: {
  /** Fills the frame; the caller sizes its own image. */
  picture: ReactNode;
  /** Top right over the picture. */
  badge?: ReactNode;
  /** 0 to 1: a thin bar along the picture's foot. */
  progress?: number | null;
  title: string;
  meta?: ReactNode;
  /** For ⌘-click to open the tile in a new tab (`newTabClicks`). */
  href: string;
  onOpen: () => void;
}) {
  return (
    <button
      type="button"
      data-tab-href={href}
      onClick={onOpen}
      className="group flex min-w-0 cursor-pointer flex-col gap-2 text-left outline-none"
    >
      <span className="relative block aspect-video w-full overflow-hidden rounded-xl border border-border bg-secondary transition-[border-color,box-shadow] group-hover:border-foreground/25 group-focus-visible:ring-[3px] group-focus-visible:ring-ring/40">
        {picture}
        {badge && <span className="absolute top-2 right-2 flex">{badge}</span>}
        {progress != null && (
          <span className="absolute inset-x-0 bottom-0 block h-1 bg-black/30">
            <span
              className="block h-full bg-brand transition-[width]"
              style={{ width: `${Math.round(Math.min(1, Math.max(0, progress)) * 100)}%` }}
            />
          </span>
        )}
      </span>
      <span className="min-w-0 space-y-0.5 px-0.5">
        <span className="block truncate text-[13px] font-medium text-foreground">{title}</span>
        {meta && <span className="block truncate text-[11.5px] text-muted-foreground">{meta}</span>}
      </span>
    </button>
  );
}
