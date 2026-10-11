export type Landing = { chunk: number; offset: number; focus: boolean };

/** The structural edits. Typing, caret, selection and undo stay the browser's;
 *  content comes back through `onEdit`. */
export interface MentionInputHandle {
  /** Swap the `@…` token starting at message offset `start` for a chip plus a space. */
  insertMention(start: number, path: string): void;
  clear(): void;
  /** Put handed-back text in front of what is typed, mentions restored to chips. */
  prepend(text: string): void;
  /** Put text after what is typed, a blank line between, as plain text. */
  append(text: string): void;
}
