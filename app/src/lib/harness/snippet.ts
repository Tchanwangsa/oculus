export interface SnippetPart {
  text: string;
  /** Inside a `**` pair: the matched term, drawn bold. */
  hit: boolean;
}

/** Splits a search snippet (`searchThreads`) on its `**` hit markers into
 *  text runs for spans, never HTML. A `**` with no partner stays literal. */
export function snippetParts(snippet: string): SnippetPart[] {
  const parts: SnippetPart[] = [];
  const push = (text: string, hit: boolean) => {
    if (!text) return;
    const last = parts[parts.length - 1];
    if (last?.hit === hit) last.text += text;
    else parts.push({ text, hit });
  };
  let at = 0;
  for (;;) {
    const open = snippet.indexOf("**", at);
    const close = open < 0 ? -1 : snippet.indexOf("**", open + 2);
    if (close < 0) break;
    push(snippet.slice(at, open), false);
    push(snippet.slice(open + 2, close), true);
    at = close + 2;
  }
  push(snippet.slice(at), false);
  return parts;
}
