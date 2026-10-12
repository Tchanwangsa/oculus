import type { ReactNode } from "react";

/**
 * The frame every video card in a page shares (`<echo-video>`,
 * `<canvas-video>`, `<web-video>`): one row of a 16:9 thumbnail, two lines of
 * text, the actions. The thumbnail sets the height, so the skeleton and every
 * state are the same size. Spans throughout, since a tag can land inside a
 * paragraph or a table cell; `md` is the tag as written, so a copied selection
 * keeps it (`lib/markdown/selection.ts`).
 */

const CARD = "my-3 flex max-w-xl select-none items-center gap-3 rounded-lg border border-border bg-surface p-2";

export function CardShell({
  md,
  thumbnail,
  title,
  titleHint,
  meta,
  actions,
}: {
  md: string;
  thumbnail: ReactNode;
  title: string;
  titleHint?: string;
  meta?: ReactNode;
  actions: ReactNode;
}) {
  return (
    <span data-md={md} className={CARD}>
      {thumbnail}
      <span className="block min-w-0 flex-1">
        <span className="block truncate text-sm font-medium text-foreground" title={titleHint ?? title}>
          {title}
        </span>
        <span className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-xs text-muted-foreground">
          {meta}
        </span>
      </span>
      <span className="flex shrink-0 items-center gap-1">{actions}</span>
    </span>
  );
}

/** A card still resolving. With the tag's `title` it shows that in place of
 *  the title bar, so the name doesn't wait on the lookup. */
export function CardSkeleton({ md, title }: { md?: string; title?: string }) {
  return (
    <span data-md={md} aria-hidden={title ? undefined : true} className={CARD}>
      <span className="block aspect-video w-32 shrink-0 animate-pulse rounded-md bg-accent" />
      <span className="block min-w-0 flex-1">
        {title ? (
          <span className="block truncate text-sm font-medium text-foreground" title={title}>
            {title}
          </span>
        ) : (
          <span className="block h-3.5 w-48 max-w-full animate-pulse rounded bg-accent" />
        )}
        <span className="mt-2 block h-3 w-16 animate-pulse rounded bg-accent" />
      </span>
    </span>
  );
}

/** An error verbatim, selectable like every error's detail. */
export function ErrorText({ children }: { children: ReactNode }) {
  return <span className="select-text break-words text-destructive">{children}</span>;
}
