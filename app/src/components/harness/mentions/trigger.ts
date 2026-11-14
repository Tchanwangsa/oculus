/** Caps on an `@` token, so a stray `@` in prose stops matching as the sentence
 *  runs on (queries may hold spaces, since filenames do). */
export const MAX_MENTION = 60;
export const MAX_MENTION_WORDS = 4;

/** What the caret is typing: an `@` mention (files and folders) or a `/` word
 *  (commands and skills). `start` is the trigger character's offset in the
 *  message; `query` is what follows it up to the caret. There is no `$`
 *  trigger, since students type `$` for maths. */
export interface ComposerTrigger {
  kind: "mention" | "slash";
  query: string;
  start: number;
}

const isSpace = (c: string) => c === " " || c === "\n" || c === "\t" || c === "\r";

/** A second slash or backslash anywhere in the word makes it a path, even past
 *  the caret (`/usr/bin`, `/etc/hosts`). T3's `isPathWord`. */
export function isPathWord(text: string, wordStart: number, caret: number): boolean {
  let end = caret;
  while (end < text.length && !isSpace(text[end])) end += 1;
  return /[/\\]/.test(text.slice(wordStart + 1, end));
}

/** The `/word` at the caret, or null: the slash starts a word (so `1/2` and
 *  `a/b` never match) and the word is not a path. T3's `detectComposerTrigger`
 *  slash branch. A picked skill reads as `/name` too, so callers pass text
 *  with chips masked (`maskedText`). */
export function slashQuery(text: string, caret: number): { query: string; start: number } | null {
  let start = caret;
  while (start > 0 && !isSpace(text[start - 1])) start -= 1;
  if (text[start] !== "/" || start >= caret) return null;
  if (isPathWord(text, start, caret)) return null;
  return { query: text.slice(start + 1, caret), start };
}

/**
 * The `@…` token at the caret, or null. Starts a word (so emails don't match)
 * and may hold spaces; "@ " closes it, the caps bound it, and a backtick ends
 * it so it can't reach back across a chip (which reads out as `` `path` ``).
 */
export function mentionQuery(text: string, caret: number): { query: string; start: number } | null {
  const m = new RegExp(`(?:^|\\s)@([^\\s@\`\\n][^@\`\\n]{0,${MAX_MENTION - 1}})?$`).exec(text.slice(0, caret));
  if (!m) return null;
  const query = m[1] ?? "";
  if (query.split(/\s+/).filter(Boolean).length > MAX_MENTION_WORDS) return null;
  return { query, start: caret - query.length - 1 };
}

/** The trigger at the caret; a `/word` wins over an `@` query that has run on
 *  past it (`@notes /mo`). */
export function detectTrigger(text: string, caret: number): ComposerTrigger | null {
  const slash = slashQuery(text, caret);
  if (slash) return { kind: "slash", ...slash };
  const mention = mentionQuery(text, caret);
  return mention ? { kind: "mention", ...mention } : null;
}
