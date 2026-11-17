/** What a `compact` composer is doing: anything true opens it into the full box. */
export interface CompactUse {
  /** Focus is somewhere in the box. */
  focused: boolean;
  /** It holds words, an attachment or a pasted card. */
  ready: boolean;
  /** Its model picker's menu is open (the menu portals out of the box). */
  picking: boolean;
  dictating: boolean;
  /** An agent's question is waiting, and the box is its answer. */
  answering: boolean;
  signingIn: boolean;
}

/** Whether a `compact` composer shows the full box rather than its resting bar. */
export function compactOpen(use: CompactUse): boolean {
  return use.focused || use.ready || use.picking || use.dictating || use.answering || use.signingIn;
}
