/**
 * Finding a cited passage on screen: in a PDF page's text layer (spans) or in
 * rendered markdown (text nodes). Both sides compare as `normalizeText` — no
 * spaces or punctuation — since a quote runs across the text layer's line
 * spans and comes from markdown. Scrolling uses offsets, never
 * `getBoundingClientRect`, which page zoom scales.
 */
import { normalizeText } from "@/lib/citations/text";

/** What to search for, best first: the whole quote, then shrinking windows of
 *  its words from the start and the middle (a quote's ends are where markdown
 *  and the PDF disagree most). */
function needles(quote: string): string[] {
  const words = quote.split(/\s+/).map(normalizeText).filter(Boolean);
  const out: string[] = [];
  const add = (ws: string[]) => {
    const n = ws.join("");
    if (n.length >= 3 && !out.includes(n)) out.push(n);
  };
  add(words);
  for (const n of [12, 8, 5]) {
    if (words.length <= n) continue;
    add(words.slice(0, n));
    const mid = Math.floor((words.length - n) / 2);
    add(words.slice(mid, mid + n));
  }
  return out;
}

/** The text-layer spans a quote covers, in order; empty if it isn't there. */
export function matchSpans(spans: HTMLElement[], quote: string): HTMLElement[] {
  let hay = "";
  const ends = spans.map((s) => (hay += normalizeText(s.textContent ?? "")).length);
  for (const needle of needles(quote)) {
    const at = hay.indexOf(needle);
    if (at < 0) continue;
    const end = at + needle.length;
    return spans.filter((_, i) => ends[i] > at && (i === 0 ? 0 : ends[i - 1]) < end);
  }
  return [];
}

/** A Range over the quote in `root`'s rendered text, or null. KaTeX output is
 *  skipped: the quote has its maths stripped. */
export function matchText(root: Node, quote: string): Range | null {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode: (n) =>
      n.parentElement?.closest(".katex") ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT,
  });
  let hay = "";
  // Per normalised character, the text node and offset it came from.
  const at: [Text, number][] = [];
  for (let n = walker.nextNode() as Text | null; n; n = walker.nextNode() as Text | null) {
    const data = n.data;
    for (let i = 0; i < data.length; ) {
      const ch = String.fromCodePoint(data.codePointAt(i)!);
      for (const c of normalizeText(ch)) {
        hay += c;
        at.push([n, i]);
      }
      i += ch.length;
    }
  }
  for (const needle of needles(quote)) {
    const i = hay.indexOf(needle);
    if (i < 0) continue;
    const [startNode, startOff] = at[i];
    const [endNode, endOff] = at[i + needle.length - 1];
    const range = document.createRange();
    range.setStart(startNode, startOff);
    range.setEnd(endNode, Math.min(endNode.data.length, endOff + 1));
    return range;
  }
  return null;
}

/** An element's top in the document in layout pixels, unscaled by page zoom
 *  and unmoved by scrolling. */
export function layoutTop(el: HTMLElement): number {
  let y = 0;
  for (let n: HTMLElement | null = el; n; n = n.offsetParent as HTMLElement | null) y += n.offsetTop;
  return y;
}

/** Scrolls `scroller` so `el` sits in its middle. Offsets are layout values,
 *  unscaled by page zoom and unmoved by scrolling. */
export function centerIn(scroller: HTMLElement, el: HTMLElement): void {
  const y = layoutTop(el) - layoutTop(scroller) - scroller.clientTop;
  scroller.scrollTop = y - (scroller.clientHeight - el.offsetHeight) / 2;
}

/** The nearest ancestor that scrolls vertically. */
export function scrollerOf(el: HTMLElement): HTMLElement | null {
  for (let n = el.parentElement; n; n = n.parentElement) {
    const o = getComputedStyle(n).overflowY;
    if (o === "auto" || o === "scroll") return n;
  }
  return null;
}
