import type { ReactNode } from "react";

import { PanelError } from "./PanelError";

/**
 * The page while History is open, in the note's place: the bar (once a
 * version shows), then its own scroller — so the hidden note's scroll
 * position is never touched — with the note's title, muted, over `children`
 * in the note's column: the version, skeletons, or the empty state. `float`
 * sits over the scroller, under the bar — the medium tier's list.
 */
export function VersionPage({
  bar,
  findBar,
  actionError,
  title,
  float,
  children,
}: {
  bar?: ReactNode;
  /** The note's find bar while it searches the version. */
  findBar?: ReactNode;
  actionError?: string | null;
  title: string;
  float?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex h-full min-h-0 flex-col">
      {bar}
      {findBar}
      {actionError && <PanelError message={actionError} />}
      <div className="relative min-h-0 flex-1">
        <div className="page-scroll">
          <div className="mx-auto w-full max-w-3xl px-6 py-8">
            <p className="font-display text-[22px] leading-tight font-semibold break-words text-muted-foreground">
              {title.trim() || "Untitled"}
            </p>
            {children}
          </div>
        </div>
        {float}
      </div>
    </div>
  );
}
