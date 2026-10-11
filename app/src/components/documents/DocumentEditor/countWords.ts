/** Runs of letters or digits, so markdown's `#`, `-` and `*` don't count;
 *  a leading frontmatter block is properties, not prose. */
export function countWords(text: string): number {
  const body = text.replace(/^---\r?\n[\s\S]*?\r?\n(?:---|\.\.\.)[ \t]*(?:\r?\n|$)/, "");
  return body.match(/[\p{L}\p{N}][\p{L}\p{N}'’_-]*/gu)?.length ?? 0;
}
