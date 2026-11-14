export type Landing = { chunk: number; offset: number; focus: boolean };

/** The structural edits. Typing, caret, selection and undo stay the browser's;
 *  content comes back through `onEdit`. */
export interface MentionInputHandle {
  /** Swap the `@…` token starting at message offset `start` for a chip plus a space. */
  insertMention(start: number, path: string): void;
  /** Swap the `/…` token starting at `start` for a skill chip plus a space. */
  insertSkill(start: number, name: string): void;
  /** Drop the token from `start` to the caret (a command that sends nothing). */
  removeToken(start: number): void;
  /** The names of the skill chips in the box, in order. */
  pickedSkills(): string[];
  clear(): void;
  /** Everything typed swapped for `text`, its chips drawn, the caret at the end. */
  replace(text: string): void;
  /** Put handed-back text in front of what is typed, mentions restored to chips. */
  prepend(text: string): void;
  /** Put text after what is typed, a blank line between, as plain text. */
  append(text: string): void;
}
