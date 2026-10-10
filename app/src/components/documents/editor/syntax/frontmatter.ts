import type { Input, TreeFragment } from "@lezer/common";
import { tags } from "@lezer/highlight";
import type { BlockContext, Line, MarkdownConfig } from "@lezer/markdown";

/**
 * YAML frontmatter for the note parser: a `---` first line, through the next
 * line that is exactly `---` or `...`. Without that closing line it is not
 * frontmatter, and the `---` stays a rule. Node: `Frontmatter`, holding two
 * `FrontmatterMark` fences; the YAML lies between them.
 *
 * The closing fence is found by looking ahead, which Lezer's fragment reuse
 * cannot see: `frontmatterFragments` drops what that lookahead invalidates.
 *
 * `parseProperties` reads the simple `key: value` subset Live mode shows; no
 * YAML library.
 */

/** How far to look for the closing fence. */
const SCAN_MAX = 64 * 1024;

const isFence = (text: string) => text === "---" || text === "...";

function frontmatter(cx: BlockContext, line: Line): boolean {
  if (cx.lineStart !== 0 || line.text !== "---") return false;
  // A block parser cannot give lines back, so find the closer before taking
  // any. `input` is runtime-only.
  const input = (cx as unknown as { input: Input }).input;
  const head = input.read(0, Math.min(input.length, SCAN_MAX));
  const lines = head.split("\n");
  // A line cut by the scan limit is not a whole line.
  const whole = head.length === input.length ? lines.length : lines.length - 1;
  let close = -1;
  for (let i = 1; i < whole; i++) {
    if (isFence(lines[i])) {
      close = i;
      break;
    }
  }
  if (close < 0) return false;

  const marks = [cx.elt("FrontmatterMark", 0, 3)];
  for (let i = 0; i < close; i++) cx.nextLine();
  const at = cx.lineStart;
  marks.push(cx.elt("FrontmatterMark", at, at + 3));
  cx.nextLine();
  cx.addElement(cx.elt("Frontmatter", 0, at + 3, marks));
  return true;
}

/**
 * `fragments` without the one from 0 when a change inside the scan could
 * make or unmake the frontmatter: with a `---` first line, Lezer would
 * otherwise reuse the old first block (a rule) without running `frontmatter`.
 * The parse then restarts at 0 and reuses fragments past the change. An old
 * `Frontmatter` first block stays, as Lezer reuses it only when the change
 * lies past its closing fence. Mirrors the core's `incremental.rs`.
 */
export function frontmatterFragments(input: Input, fragments: readonly TreeFragment[]): readonly TreeFragment[] {
  const first = fragments[0];
  if (!first || first.from > 0 || first.to > SCAN_MAX) return fragments;
  const head = input.read(0, Math.min(input.length, 4));
  if (head !== "---\n" && head !== "---") return fragments;
  if (first.tree.topNode.firstChild?.name === "Frontmatter") return fragments;
  return fragments.slice(1);
}

export const FrontmatterSyntax: MarkdownConfig = {
  defineNodes: [
    { name: "Frontmatter", block: true, style: tags.special(tags.content) },
    { name: "FrontmatterMark", style: tags.processingInstruction },
  ],
  parseBlock: [{ name: "Frontmatter", parse: frontmatter, before: "HorizontalRule" }],
};
export type PropertyValue =
  | { kind: "text"; text: string }
  | { kind: "list"; items: string[] }
  | { kind: "raw"; text: string };

export interface Property {
  /** Null for a line that isn't `key: value`, shown as written. */
  key: string | null;
  value: PropertyValue;
  /** Where the property's line starts in the frontmatter source. */
  at: number;
}

function unquote(s: string): string {
  if (s.length >= 2 && s.startsWith('"') && s.endsWith('"')) {
    try {
      return JSON.parse(s) as string;
    } catch {
      return s.slice(1, -1);
    }
  }
  if (s.length >= 2 && s.startsWith("'") && s.endsWith("'")) return s.slice(1, -1).replace(/''/g, "'");
  return s;
}

/** `[a, "b, c", d]` into its items, quotes respected. */
function flowList(inner: string): string[] {
  const items: string[] = [];
  let cur = "";
  let quote = "";
  for (const ch of inner) {
    if (quote) {
      cur += ch;
      if (ch === quote) quote = "";
    } else if (ch === '"' || ch === "'") {
      quote = ch;
      cur += ch;
    } else if (ch === ",") {
      items.push(cur);
      cur = "";
    } else {
      cur += ch;
    }
  }
  items.push(cur);
  return items.map((s) => unquote(s.trim())).filter((s) => s !== "");
}

/** A plain scalar loses a trailing ` # comment`; a quoted one its quotes. */
function scalar(value: string): string {
  if (/^["']/.test(value)) return unquote(value);
  return value.replace(/\s+#.*$/, "");
}

const KEY = /^("[^"]*"|'[^']*'|[^\s#:\-[{][^:]*?)\s*:(?:\s+(.*?))?\s*$/;
const ITEM = /^\s*-(?:\s+(.*?))?\s*$/;
const BLOCK_SCALAR = /^[|>][-+0-9]*$/;

/** The properties in a frontmatter block's source, fences included. */
export function parseProperties(source: string): Property[] {
  const lines = source.split("\n");
  const starts: number[] = [];
  let pos = 0;
  for (const text of lines) {
    starts.push(pos);
    pos += text.length + 1;
  }
  const end = lines.length - 1;
  const props: Property[] = [];
  for (let i = 1; i < end; i++) {
    const text = lines[i];
    if (!text.trim() || /^\s*#/.test(text)) continue;
    const m = KEY.exec(text);
    if (!m) {
      props.push({ key: null, value: { kind: "raw", text: text.trim() }, at: starts[i] });
      continue;
    }
    const key = unquote(m[1]);
    const value = m[2] ?? "";
    const at = starts[i];
    // Indented lines and `- item` lines belong to this key.
    const more: string[] = [];
    while (i + 1 < end && (/^[ \t]+\S/.test(lines[i + 1]) || ITEM.test(lines[i + 1]))) more.push(lines[++i]);

    let parsed: PropertyValue;
    if (!value && more.length && more.every((l) => ITEM.test(l))) {
      parsed = { kind: "list", items: more.map((l) => unquote(ITEM.exec(l)?.[1] ?? "")).filter(Boolean) };
    } else if (BLOCK_SCALAR.test(value)) {
      const indent = Math.min(...more.map((l) => /^\s*/.exec(l)![0].length));
      const body = more.map((l) => l.slice(indent));
      parsed = { kind: "text", text: body.join(value.startsWith("|") ? "\n" : " ") };
    } else if (more.length) {
      parsed = { kind: "raw", text: [value, ...more.map((l) => l.trim())].filter(Boolean).join("\n") };
    } else if (value.startsWith("[") && value.endsWith("]")) {
      parsed = { kind: "list", items: flowList(value.slice(1, -1)) };
    } else {
      parsed = { kind: "text", text: scalar(value) };
    }
    props.push({ key, value: parsed, at });
  }
  return props;
}
