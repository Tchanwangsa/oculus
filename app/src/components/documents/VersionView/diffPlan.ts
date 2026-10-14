import type { EditorState } from "@codemirror/state";

import type { DiffChange, DiffHunk } from "@/lib/notes/versionDiff";

import { constructsIn, type Construct } from "./constructs";
import { visibleIn } from "./visible";

/**
 * Where each change's marks go on the shown text, before they become
 * decorations (`diffDecorations.ts`): inserted spans, deleted text with its
 * anchor, widget constructs a change falls in (tinted whole, `ins` when the
 * construct is all new text), and the kind of each touched line.
 * Each piece carries its hunk's index, for the current hunk's flash.
 */

export type LineKind = "ins" | "del" | "mix";

export interface DiffPlan {
  inserts: { from: number; to: number; hunk: number }[];
  /** `block` text spans lines and draws between them, before the line at
   *  `at` (`side` -1) or after it (1). */
  deletions: { at: number; text: string; block: boolean; side: -1 | 1; hunk: number }[];
  constructs: (Construct & { kind: "ins" | "mod"; hunk: number })[];
  /** By line start; never a line inside a block construct, which draws its
   *  own bar. */
  lines: { at: number; kind: LineKind; hunks: number[] }[];
}

/** The hunk holding `c`, by position: hunks are sorted and disjoint. */
function hunkOf(hunks: readonly DiffHunk[], c: DiffChange): number {
  const i = hunks.findIndex((h) => h.fromB <= c.fromB && c.toB <= h.toB);
  return i < 0 ? 0 : i;
}

/** `from..to` less the spans in `cut`, in order. */
function subtract(from: number, to: number, cut: readonly { from: number; to: number }[]) {
  const out: { from: number; to: number }[] = [];
  let at = from;
  for (const c of [...cut].sort((a, b) => a.from - b.from)) {
    if (c.to <= at || c.from >= to) continue;
    if (c.from > at) out.push({ from: at, to: c.from });
    at = Math.max(at, c.to);
  }
  if (at < to) out.push({ from: at, to });
  return out;
}

/** The constructs `c` folds into: those it overlaps, and an inline one it
 *  only touches when nothing visible changed beside it (a space, a hidden
 *  delimiter), so that change draws no box of its own. */
function absorbing(state: EditorState, c: DiffChange, deleted: string): Construct[] {
  const hit = constructsIn(state, c.fromB, c.toB);
  const len = state.doc.length;
  const near = constructsIn(state, Math.max(0, c.fromB - 1), Math.min(len, c.toB + 1)).filter(
    (k) => !k.block && k.from <= c.toB && k.to >= c.fromB && !hit.some((h) => h.from === k.from && h.to === k.to),
  );
  if (!near.length || visibleIn(state, c.fromB, c.toB) || /[^\s$\\()[\]`]/.test(deleted)) return hit;
  return [...hit, ...near].sort((a, b) => a.from - b.from);
}

/** `part` (beside a construct) without the whitespace on the side that
 *  meets one. */
function trimToward(state: EditorState, part: { from: number; to: number }, hit: readonly Construct[]) {
  let { from, to } = part;
  if (hit.some((k) => k.from === to)) while (to > from && /\s/.test(state.sliceDoc(to - 1, to))) to--;
  if (hit.some((k) => k.to === from)) while (from < to && /\s/.test(state.sliceDoc(from, from + 1))) from++;
  return { from, to };
}

/** A construct's opening and closing delimiters, as its source shows them. */
function delimiters(state: EditorState, k: Construct): [string, string] {
  const src = state.sliceDoc(k.from, k.to);
  const open = /^(\\\(|!\[|\$\$|`+|\$|\[)/.exec(src)?.[0] ?? src.slice(0, 1);
  const close = /(\\\)|\$\$|`+|\$|\)|\])$/.exec(src)?.[0] ?? src.slice(-1);
  return [open, close];
}

/** The parts of `deleted` that lay outside the inline constructs `c` runs
 *  into: before the first one's opening delimiter and after the last one's
 *  closing one, found in the old text by those delimiters. A delimiter the
 *  old text lacks was typed new, so everything went from that side. */
function outsideDeleted(state: EditorState, c: DiffChange, deleted: string, hit: readonly Construct[]) {
  const none = { before: "", after: "", at: 0 };
  if (hit.some((k) => k.block)) return none;
  const first = hit[0];
  const last = hit[hit.length - 1];
  const open = c.fromB <= first.from ? deleted.indexOf(delimiters(state, first)[0]) : 0;
  const before = open < 0 ? deleted : deleted.slice(0, open);
  let after = "";
  if (c.toB >= last.to && open >= 0) {
    const close = delimiters(state, last)[1];
    const at = deleted.lastIndexOf(close);
    after = at < 0 ? deleted.slice(open) : deleted.slice(at + close.length);
  }
  return { before, after, at: last.to };
}

/** Inserted spans sorted, with those that meet or overlap in one hunk merged. */
function mergeAbutting(spans: DiffPlan["inserts"]): DiffPlan["inserts"] {
  const out: DiffPlan["inserts"] = [];
  for (const s of [...spans].sort((a, b) => a.from - b.from)) {
    const prev = out[out.length - 1];
    if (prev && prev.hunk === s.hunk && s.from <= prev.to) prev.to = Math.max(prev.to, s.to);
    else out.push({ ...s });
  }
  return out;
}

export function diffPlan(state: EditorState, base: string, changes: readonly DiffChange[], hunks: readonly DiffHunk[]): DiffPlan {
  const { doc } = state;
  const plan: DiffPlan = { inserts: [], deletions: [], constructs: [], lines: [] };
  const lineKinds = new Map<number, { ins: boolean; del: boolean; hunks: Set<number> }>();
  const touch = (pos: number, kind: "ins" | "del", hunk: number) => {
    const at = doc.lineAt(pos).from;
    const entry = lineKinds.get(at) ?? { ins: false, del: false, hunks: new Set<number>() };
    entry[kind] = true;
    entry.hunks.add(hunk);
    lineKinds.set(at, entry);
  };
  const seen = new Set<string>();
  const inserts: DiffPlan["inserts"] = [];

  for (const c of changes) {
    const hunk = hunkOf(hunks, c);
    let deleted = base.slice(c.fromA, c.toA);
    const hit = absorbing(state, c, deleted);
    for (const k of hit) {
      const key = `${k.from}:${k.to}`;
      if (seen.has(key)) continue;
      seen.add(key);
      const kind = c.fromB <= k.from && k.to <= c.toB && !deleted.trim() ? "ins" : "mod";
      plan.constructs.push({ ...k, kind, hunk });
      if (!k.block) {
        touch(k.from, "ins", hunk);
        if (kind === "mod") touch(k.from, "del", hunk);
      }
    }
    for (const part of subtract(c.fromB, c.toB, hit)) {
      // The inserted text's lines; a closing newline ends the last one.
      const last = doc.lineAt(Math.max(part.from, part.to - 1)).number;
      for (let n = doc.lineAt(part.from).number; n <= last; n++) touch(doc.line(n).from, "ins", hunk);
      const span = hit.length ? trimToward(state, part, hit) : part;
      if (visibleIn(state, span.from, span.to)) inserts.push({ ...span, hunk });
    }
    if (hit.length) {
      // What went from outside the construct still shows, the rest is its
      // tint. The tail goes just past it (side 1 is outside its mark).
      const out = outsideDeleted(state, c, deleted, hit);
      if (out.after.trim()) plan.deletions.push({ at: out.at, text: out.after, block: false, side: 1, hunk });
      if (!out.before.trim()) continue;
      deleted = out.before;
    }
    if (!deleted || constructsIn(state, c.fromB, c.fromB).length) continue;
    if (!deleted.includes("\n")) {
      // Struck-out spaces alone would draw as an empty box.
      if (deleted.trim()) plan.deletions.push({ at: c.fromB, text: deleted, block: false, side: -1, hunk });
      touch(c.fromB, "del", hunk);
      continue;
    }
    const line = doc.lineAt(c.fromB);
    const text = deleted.replace(/^\n+|\n+$/g, "");
    if (!text.trim()) {
      // A bare line break: a mark where it was, not an empty block.
      plan.deletions.push({ at: c.fromB, text: "¶", block: false, side: -1, hunk });
      touch(c.fromB, "del", hunk);
    } else if (c.fromB === line.from) {
      // A block construct's wrapper takes a block widget at its start, so
      // the lines go after the line above instead.
      const above = line.number > 1 && constructsIn(state, line.from, line.from + 1).some((k) => k.block && k.from === line.from);
      if (above) plan.deletions.push({ at: line.from - 1, text, block: true, side: 1, hunk });
      else plan.deletions.push({ at: line.from, text, block: true, side: -1, hunk });
    } else {
      plan.deletions.push({ at: line.to, text, block: true, side: 1, hunk });
      // Text on the line it hangs from went too.
      if (!deleted.startsWith("\n")) touch(c.fromB, "del", hunk);
    }
  }

  plan.inserts = mergeAbutting(inserts);
  const blocks = plan.constructs.filter((k) => k.block);
  for (const [at, e] of [...lineKinds].sort((a, b) => a[0] - b[0])) {
    if (blocks.some((k) => k.from <= at && at <= k.to)) continue;
    plan.lines.push({ at, kind: e.ins && e.del ? "mix" : e.ins ? "ins" : "del", hunks: [...e.hunks] });
  }
  return plan;
}
