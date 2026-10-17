/**
 * Web links in a message: a bare `https://…` URL or a markdown
 * `[words](https://…)`. The composer draws each as a reference
 * (`LinkChip`) and the bubble does too, while the wire keeps the text as
 * written (`raw`), so the agent reads what the student typed.
 */

export type LinkPart = { kind: "text"; text: string } | { kind: "link"; raw: string };

/** Code spans come first so a URL inside backticks stays code. Trailing
 *  sentence punctuation is not part of a bare URL. */
const LINK = /`[^`\n]*`|\[([^\]\n]+)\]\((https?:\/\/[^\s)]+)\)|https?:\/\/[^\s<>()`]*[^\s<>()`.,;:!?'"]/g;

/** What a link's `raw` points at and says; null if it is not a link. */
export function parseLink(raw: string): { href: string; label: string | null } | null {
  const md = /^\[([^\]\n]+)\]\((https?:\/\/[^\s)]+)\)$/.exec(raw);
  if (md) return { href: md[2], label: md[1] };
  return /^https?:\/\/\S+$/.test(raw) ? { href: raw, label: null } : null;
}

/**
 * Splits text into prose and links. `settled` keeps only links followed by
 * whitespace, so a URL still being typed is left alone until the space after
 * it; a paste or a restored draft passes false and takes them all.
 */
export function splitLinks(text: string, settled = false): LinkPart[] {
  const parts: LinkPart[] = [];
  let at = 0;
  for (const m of text.matchAll(LINK)) {
    if (m[0].startsWith("`")) continue;
    const end = (m.index ?? 0) + m[0].length;
    if (settled && !/\s/.test(text[end] ?? "")) continue;
    const i = m.index ?? 0;
    if (i > at) parts.push({ kind: "text", text: text.slice(at, i) });
    parts.push({ kind: "link", raw: m[0] });
    at = end;
  }
  if (at < text.length) parts.push({ kind: "text", text: text.slice(at) });
  return parts;
}
