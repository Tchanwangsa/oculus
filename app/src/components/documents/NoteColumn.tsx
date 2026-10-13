import type { ReactNode } from "react";

/**
 * The page scroller and a note's column, shared by the note and the History
 * version over it so both read the same. The column is fixed at the note's
 * widest and never narrower: a pane smaller than that scrolls sideways instead
 * of re-wrapping the text. Auto margins centre it and give way to 0 once it
 * overflows, so the start edge stays reachable.
 */
export function NoteColumn({ children }: { children: ReactNode }) {
  return (
    <div className="page-scroll overflow-x-auto">
      <div className="mx-auto w-3xl px-6 py-8">{children}</div>
    </div>
  );
}
