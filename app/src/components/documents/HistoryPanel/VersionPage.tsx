import type { ReactNode } from "react";

import { NoteColumn } from "../NoteColumn";
import { PanelError } from "./PanelError";

/**
 * The page while History is open, in the note's place: the bar, then its own
 * scroller — so the hidden note's scroll position is never touched — with the
 * note's title, muted, over `children` in the note's column: the version,
 * skeletons, or the empty state. The bar stays put while the column scrolls
 * sideways (`NoteColumn`).
 */
export function VersionPage({
  bar,
  findBar,
  errors,
  title,
  children,
}: {
  bar: ReactNode;
  /** The note's find bar while it searches the version. */
  findBar?: ReactNode;
  /** Failed reads and actions, over the text. */
  errors: (string | null)[];
  title: string;
  children: ReactNode;
}) {
  return (
    <div className="flex h-full min-h-0 flex-col">
      {bar}
      {findBar}
      {errors.map((message, i) => message && <PanelError key={i} message={message} />)}
      <div className="relative min-h-0 flex-1">
        <NoteColumn>
          <p className="font-display text-[22px] leading-tight font-semibold break-words text-muted-foreground">
            {title.trim() || "Untitled"}
          </p>
          {children}
        </NoteColumn>
      </div>
    </div>
  );
}
