import { attachmentPath } from "@/lib/harness/attachments";
import { parseLink, splitLinks } from "@/lib/harness/links";
import { skillText } from "@/lib/harness/skills";

/** Runs of text with atomic mentions (a mention is only its path), atomic
 *  attachments (only their `./attachments/…` path, as the message spells
 *  it), atomic skills (only their name) and links (their text as written, which the caret
 *  edits like any other). The DOM, not this model, is the truth while typing:
 *  React renders it once per structural change, because a contenteditable
 *  re-rendered per keystroke drops the caret. */
export type Chunk =
  | { kind: "text"; text: string }
  | { kind: "chip"; path: string }
  | { kind: "attachment"; path: string }
  | { kind: "skill"; name: string }
  | { kind: "link"; raw: string };

/** A mention as the message spells it: the backticked path `oculus read` takes. */
export function chipText(path: string): string {
  return `\`${path}\``;
}

/** Caret landing strip: WebKit cannot place a caret before a
 *  `contenteditable="false"` that starts a block. Stripped from every read. */
const ZWSP = "​";
export const ZWSP_RE = /​/g;

export function chunkText(c: Chunk): string {
  if (c.kind === "chip" || c.kind === "attachment") return chipText(c.path);
  if (c.kind === "skill") return skillText(c.name);
  return c.kind === "link" ? c.raw : c.text.replace(ZWSP_RE, "");
}

export function chunksText(chunks: Chunk[]): string {
  return chunks.map(chunkText).join("");
}

/** Stands in for each character of a picked skill in `maskedText`. */
const MASK = "\ufffc";

/** `chunksText` with every picked skill's characters masked, same length: a
 *  chip reads as `/name`, which must not open the `/` menu again. */
export function maskedText(chunks: Chunk[]): string {
  return chunks.map((c) => (c.kind === "skill" ? MASK.repeat(chunkText(c).length) : chunkText(c))).join("");
}

/** No empty or adjacent text runs (the caret maths addresses one DOM node per
 *  chunk by index), plus a ZWSP guard before a leading mention or skill. */
export function normalize(chunks: Chunk[]): Chunk[] {
  const out: Chunk[] = [];
  for (const c of chunks) {
    if (c.kind !== "text") {
      out.push(c);
      continue;
    }
    const text = c.text.replace(ZWSP_RE, "");
    if (!text) continue;
    const last = out[out.length - 1];
    if (last?.kind === "text") out[out.length - 1] = { kind: "text", text: last.text + text };
    else out.push({ kind: "text", text });
  }
  if (out[0] && out[0].kind !== "text" && out[0].kind !== "link") out.unshift({ kind: "text", text: ZWSP });
  return out;
}

/** DOM offset of the nth message character in a chunk, skipping ZWSP guards. */
export function domOffset(text: string, n: number): number {
  let seen = 0;
  for (let i = 0; i < text.length; i++) {
    if (seen === n && text[i] !== ZWSP) return i;
    if (text[i] !== ZWSP) seen++;
  }
  return text.length;
}

/** A message offset as (chunk, offset) — chunks, since the old nodes are gone by
 *  the time the caret lands and `childNodes[chunk]` is what that chunk became.
 *  An offset inside a mention resolves to just after it; one inside a link
 *  lands in its drawn text (a markdown link's words). */
export function caretChunk(chunks: Chunk[], offset: number): { chunk: number; offset: number } {
  let at = 0;
  for (let i = 0; i < chunks.length; i++) {
    const c = chunks[i];
    const len = chunkText(c).length;
    if (c.kind === "link" && offset < at + len) {
      const label = parseLink(c.raw)?.label;
      const inner = offset - at;
      return { chunk: i, offset: label == null ? inner : Math.max(0, Math.min(label.length, inner - 1)) };
    }
    if (c.kind !== "text") {
      if (offset <= at + len) return { chunk: i + 1, offset: 0 };
    } else if (offset <= at + len) {
      return { chunk: i, offset: domOffset(c.text, offset - at) };
    }
    at += len;
  }
  return { chunk: chunks.length, offset: 0 };
}

/** How far `linkChunks` looks for new links in prose: every link (`all`, a
 *  paste or restored draft), those a typed space has ended (`settled`, see
 *  `splitLinks`), or none (`typed`, a keystroke mid-word). */
export type LinkScan = "all" | "settled" | "typed";

/** The links in the text, as chunks; the message text does not change. An
 *  existing link is checked again, as typing may have reached into it: a
 *  space or trailing comma splits it, and one no longer a link is prose. */
export function linkChunks(chunks: Chunk[], scan: LinkScan = "all"): Chunk[] {
  return chunks.flatMap((c): Chunk[] => {
    if (c.kind === "link") return splitLinks(c.raw);
    if (c.kind === "text" && scan !== "typed") return splitLinks(c.text, scan === "settled");
    return [c];
  });
}

/** Whether two chunk lists draw the same nodes. */
export function sameChunks(a: Chunk[], b: Chunk[]): boolean {
  return (
    a.length === b.length &&
    a.every(
      (c, i) =>
        c.kind === b[i].kind &&
        (c.kind !== "link" || c.raw === (b[i] as typeof c).raw) &&
        (c.kind !== "attachment" || c.path === (b[i] as typeof c).path) &&
        (c.kind !== "skill" || c.name === (b[i] as typeof c).name),
    )
  );
}

/** The chunks before and after message offset `offset`; a chip the offset
 *  falls inside goes before. */
function splitAt(chunks: Chunk[], offset: number): [Chunk[], Chunk[]] {
  let at = 0;
  for (let i = 0; i < chunks.length; i++) {
    const c = chunks[i];
    const len = chunkText(c).length;
    if (offset < at + len || (c.kind === "text" && offset === at + len)) {
      if (c.kind !== "text") return [chunks.slice(0, i + 1), chunks.slice(i + 1)];
      const cut = domOffset(c.text, offset - at);
      return [
        [...chunks.slice(0, i), { kind: "text", text: c.text.slice(0, cut) }],
        [{ kind: "text", text: c.text.slice(cut) }, ...chunks.slice(i + 1)],
      ];
    }
    at += len;
  }
  return [chunks, []];
}

/** `insert` placed at message offset `offset` (the caret), each chip a space
 *  from its neighbours, and the caret after the last and its space. */
export function insertAt(chunks: Chunk[], offset: number, insert: Chunk[]): { chunks: Chunk[]; caret: number } {
  const [before, after] = splitAt(chunks, offset);
  const head = chunksText(before);
  const tail = chunksText(after);
  const added: Chunk[] = [];
  if (head && !/\s$/.test(head)) added.push({ kind: "text", text: " " });
  insert.forEach((c, i) => {
    if (i) added.push({ kind: "text", text: " " });
    added.push(c);
  });
  if (!/^\s/.test(tail)) added.push({ kind: "text", text: " " });
  const next = normalize([...before, ...added, ...after]);
  return { chunks: next, caret: head.length + chunksText(added).length };
}

/** Every chip of the attachment `raw` (any spelling) taken out, one space
 *  closing each gap; the caret moves back by what went before it. */
export function dropAttachment(chunks: Chunk[], raw: string, caret: number): { chunks: Chunk[]; caret: number } {
  const target = attachmentPath(raw);
  // The guard is not text; `normalize` puts it back where it is needed.
  const plain = chunks.map((c): Chunk => (c.kind === "text" ? { kind: "text", text: c.text.replace(ZWSP_RE, "") } : c));
  const text = chunksText(plain);
  const cuts: [number, number][] = [];
  let at = 0;
  for (const c of plain) {
    const len = chunkText(c).length;
    if (c.kind === "attachment" && attachmentPath(c.path) === target) {
      let [from, to] = [at, at + len];
      const before = text.slice(0, from);
      const after = text.slice(to);
      if (after.startsWith(" ") && (!before || /\s$/.test(before))) to += 1;
      else if (before.endsWith(" ") && (!after || after.startsWith("\n"))) from -= 1;
      cuts.push([from, to]);
    }
    at += len;
  }
  if (!cuts.length) return { chunks, caret };
  const out: Chunk[] = [];
  at = 0;
  for (const c of plain) {
    const len = chunkText(c).length;
    const start = at;
    at += len;
    if (c.kind !== "text") {
      if (!cuts.some(([from, to]) => from <= start && start + len <= to)) out.push(c);
      continue;
    }
    let kept = "";
    for (let k = 0; k < c.text.length; k++) {
      const pos = start + k;
      if (!cuts.some(([from, to]) => pos >= from && pos < to)) kept += c.text[k];
    }
    out.push({ kind: "text", text: kept });
  }
  let moved = caret;
  for (const [from, to] of merge(cuts)) {
    if (caret >= to) moved -= to - from;
    else if (caret > from) moved -= caret - from;
  }
  return { chunks: normalize(out), caret: moved };
}

/** Ranges in order, overlapping ones joined. */
function merge(ranges: [number, number][]): [number, number][] {
  const out: [number, number][] = [];
  for (const [from, to] of [...ranges].sort((a, b) => a[0] - b[0])) {
    const last = out[out.length - 1];
    if (last && from <= last[1]) last[1] = Math.max(last[1], to);
    else out.push([from, to]);
  }
  return out;
}
