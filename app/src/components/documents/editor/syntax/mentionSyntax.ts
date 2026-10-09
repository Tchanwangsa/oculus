import { parseCitation } from "@/lib/citations";

/**
 * How a note spells a file mention, and how Live mode knows one back: the
 * backticked library path the chat composer sends, which `@` writes
 * (`mentions.ts`) and `CitationWidget` draws as a chip. Any inline code span
 * that is wholly a citation counts, as it does in a chat reply.
 */

/** What `@` writes for the file at `path`. */
export function mentionText(path: string): string {
  return `\`${path}\``;
}

/** The citation inside an inline code span's source (backticks included),
 *  or null when it is ordinary code. One line only: a plugin's decoration
 *  cannot replace a line break. */
export function inlineCodeCitation(source: string): string | null {
  const run = /^`+/.exec(source)?.[0].length ?? 0;
  if (!run || source.length <= run * 2 || !source.endsWith("`".repeat(run))) return null;
  const text = source.slice(run, -run);
  if (text.includes("\n")) return null;
  return parseCitation(text) ? text.trim() : null;
}
