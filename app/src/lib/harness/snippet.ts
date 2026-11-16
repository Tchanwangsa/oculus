export interface SnippetPart {
  text: string;
  /** Between a hit pair: the matched term, drawn bold. */
  hit: boolean;
}

/** Hit markers `searchThreads` asks FTS to wrap matches in. Private-use
 *  characters, so a message's own `**bold**` is never taken for a hit. */
export const HIT_OPEN = "\uE000";
export const HIT_CLOSE = "\uE001";

/** Markdown emphasis and code marks, dropped so a snippet reads as plain text. */
const MARKS = /[*`~]+/g;

/** Splits a search snippet (`searchThreads`) on its hit markers into text
 *  runs for spans, never HTML. A marker with no partner is dropped. */
export function snippetParts(snippet: string): SnippetPart[] {
  const parts: SnippetPart[] = [];
  const push = (raw: string, hit: boolean) => {
    const text = raw.replace(MARKS, "").split(HIT_OPEN).join("").split(HIT_CLOSE).join("");
    if (!text) return;
    const last = parts[parts.length - 1];
    if (last?.hit === hit) last.text += text;
    else parts.push({ text, hit });
  };
  let at = 0;
  for (;;) {
    const open = snippet.indexOf(HIT_OPEN, at);
    const close = open < 0 ? -1 : snippet.indexOf(HIT_CLOSE, open + 1);
    if (close < 0) break;
    push(snippet.slice(at, open), false);
    push(snippet.slice(open + 1, close), true);
    at = close + 1;
  }
  push(snippet.slice(at), false);
  return parts;
}
