// Checks the Rust `ChangeSet`/`ChangeDesc`/`Selection` against
// @codemirror/state. Each case is a random document with three random change
// sets — `a` and `c` on the document, `b` on `a`'s result — built from spec
// lists that are sometimes out of order, overlapping or nested sets. The
// answer covers toJSON of each, apply, invert, compose, map (both `before`s,
// set and desc), mapPos at every position in every mode and assoc,
// iterChanges/iterGaps/iterChangedRanges, touchesRange, and random selections:
// normalised, mapped through `a`, `extend`/`asSingle`/`addRange`/
// `replaceRange`, and `changeByRange` with three recipes. Change sets and
// descs are also read back from their `toJSON` (CodeMirror's own, and raw
// JSON with unmerged sections that `fromJSON` keeps as given), and raw sets
// composed and mapped, refused exactly where CodeMirror throws.
//
//   bun editor-core/oracle/changes.ts [cases] [seed] [only]     (from app/)

import {
  ChangeDesc,
  ChangeSet,
  EditorSelection,
  EditorState,
  MapMode,
  type SelectionRange,
  type Text,
} from "@codemirror/state";

import { SPLIT, boundary, source, textOf } from "./docs";
import { Checker, type Rng, buildOracle, caseRng, parseArgs } from "./driver";

const args = parseArgs(20000);
const script = "bun editor-core/oracle/changes.ts";

type Spec = [number, number, string] | { set: Spec[] };

/** A short insert: empty a third of the time, sometimes with breaks. */
function insertText(rng: Rng): string {
  if (rng.chance(0.3)) return "";
  return source(rng, rng.logInt(1, 12), 0.15);
}

/** Random specs on `doc`: mostly ascending, sometimes out of order or
 * overlapping (which `ChangeSet.of` composes), sometimes a nested set. */
function specs(rng: Rng, doc: Text, depth = 0): Spec[] {
  const n = rng.pick([0, 1, 1, 2, 3, 5, 8]);
  const out: Spec[] = [];
  let pos = 0;
  for (let i = 0; i < n; i++) {
    if (depth === 0 && rng.chance(0.06)) {
      out.push({ set: specs(rng, doc, 1) });
      continue;
    }
    const ordered = rng.chance(0.75);
    let from = ordered ? Math.min(doc.length, pos + rng.int(8)) : boundary(rng, doc);
    if (ordered && from > 0 && from < doc.length) {
      const c = doc.sliceString(from, from + 1).charCodeAt(0);
      if (c >= 0xdc00 && c <= 0xdfff) from--;
    }
    let to = Math.min(doc.length, from + rng.pick([0, 0, 1, 2, 3, 6, 20]));
    if (to > 0 && to < doc.length) {
      const c = doc.sliceString(to, to + 1).charCodeAt(0);
      if (c >= 0xdc00 && c <= 0xdfff) to++;
    }
    out.push([from, to, insertText(rng)]);
    pos = to;
  }
  return out;
}

function toCm(list: Spec[], length: number): any[] {
  return list.map((s) =>
    Array.isArray(s) ? { from: s[0], to: s[1], insert: s[2] } : ChangeSet.of(toCm(s.set, length), length),
  );
}

const MODES = [MapMode.Simple, MapMode.TrackDel, MapMode.TrackBefore, MapMode.TrackAfter];

type RangeSpec = [number, number, number | null, number | null, number];

function randomSelection(rng: Rng, doc: Text): { ranges: RangeSpec[]; main: number } {
  const n = rng.pick([1, 1, 2, 3, 5]);
  const ranges: RangeSpec[] = [];
  for (let i = 0; i < n; i++) {
    const anchor = boundary(rng, doc);
    const head = rng.chance(0.4) ? anchor : rng.chance(0.5) ? boundary(rng, doc) : Math.min(doc.length, anchor + rng.int(4));
    const goal = rng.chance(0.2) ? rng.pick([0, 3, 12.5, 200]) : null;
    const bidi = rng.chance(0.15) ? rng.int(8) : null;
    const assoc = rng.chance(0.3) ? rng.pick([-1, 1]) : 0;
    ranges.push([anchor, head, goal, bidi, assoc]);
  }
  return { ranges, main: rng.int(n) };
}

function cmSelection(s: { ranges: RangeSpec[]; main: number }): EditorSelection {
  return EditorSelection.create(s.ranges.map(cmRange), s.main);
}

const rangeJson = (r: SelectionRange) => [r.anchor, r.head, r.assoc, r.goalColumn ?? null, r.bidiLevel];

export function selectionJson(sel: EditorSelection) {
  return { ranges: sel.ranges.map(rangeJson), main: sel.mainIndex };
}

const cmRange = ([a, h, goal, bidi, assoc]: RangeSpec) =>
  EditorSelection.range(a, h, goal ?? undefined, bidi ?? undefined, assoc || undefined);

/** `changeByRange` recipes, as `by_range` in src/bin/oracle/changes.rs. */
function byRange(doc: Text, sel: EditorSelection, recipe: string) {
  const state = EditorState.create({ doc, selection: sel, extensions: EditorState.allowMultipleSelections.of(true) });
  const out = state.changeByRange((r) => {
    if (recipe === "delete_back") {
      let [from, to] = [r.from, r.to];
      if (r.empty) {
        const c = r.head > 0 ? doc.sliceString(r.head - 1, r.head).charCodeAt(0) : 0;
        from = r.head - (r.head >= 2 && c >= 0xdc00 && c <= 0xdfff ? 2 : Math.min(r.head, 1));
      }
      return { changes: { from, to }, range: EditorSelection.cursor(from) };
    }
    if (recipe === "wrap") {
      return {
        changes: [{ from: r.from, insert: "**" }, { from: r.to, insert: "**" }],
        range: EditorSelection.range(r.anchor + 2, r.head + 2),
      };
    }
    return { changes: { from: r.head, insert: "ไ😀" }, range: EditorSelection.cursor(r.head + 3) };
  });
  return [(out.changes as ChangeSet).toJSON(), selectionJson(out.selection)];
}

const RECIPES = ["delete_back", "wrap", "insert_head"];

function randomRange(rng: Rng, doc: Text): RangeSpec {
  const anchor = boundary(rng, doc);
  const head = rng.chance(0.4) ? anchor : boundary(rng, doc);
  return [anchor, head, rng.chance(0.2) ? 5 : null, rng.chance(0.1) ? rng.int(3) : null, rng.chance(0.3) ? rng.pick([-1, 1]) : 0];
}

/** Raw `ChangeSet.toJSON` over `doc`, cut at code-point boundaries:
 * keeps, deletions and replacements, sometimes empty or side by side. */
function rawSetJson(rng: Rng, doc: Text): unknown[] {
  const cuts = [...new Set(Array.from({ length: rng.int(6) }, () => boundary(rng, doc)))].sort((x, y) => x - y);
  const out: unknown[] = [];
  let pos = 0;
  for (const cut of [...cuts, doc.length]) {
    if (rng.chance(0.15)) out.push(rng.chance(0.5) ? 0 : [0, ...source(rng, rng.logInt(1, 6), 0.3).split(SPLIT)]);
    const len = cut - pos;
    if (len === 0 && cut !== doc.length) continue;
    const r = rng.next();
    if (r < 0.5) out.push(len);
    else if (r < 0.65) out.push([len]);
    else out.push([len, ...source(rng, rng.logInt(1, 10), 0.3).split(SPLIT)]);
    pos = cut;
  }
  return out;
}

/** What src/bin/oracle/changes.rs answers for a set read back from JSON,
 * with the document it applies to. */
function jsonSet([json, on]: [unknown, string]) {
  const set = ChangeSet.fromJSON(json);
  const doc = textOf(on);
  return [set.toJSON(), set.length, set.newLength, set.desc.toJSON(), set.apply(doc).toString(), set.invert(doc).toJSON()];
}

/** `f()`, or "throw" where CodeMirror throws. */
function attempt(f: () => unknown): unknown {
  try {
    return f();
  } catch {
    return "throw";
  }
}

/** What src/bin/oracle/changes.rs answers for raw sets `a` and `c` on one
 * document and `b` on `a`'s result. */
function rawOps(j: { a: unknown; b: unknown; c: unknown }) {
  const [a, b, c] = [ChangeSet.fromJSON(j.a), ChangeSet.fromJSON(j.b), ChangeSet.fromJSON(j.c)];
  return [
    attempt(() => a.compose(b).toJSON()),
    attempt(() => a.map(c).toJSON()),
    attempt(() => a.map(c, true).toJSON()),
    attempt(() => a.desc.composeDesc(b.desc).toJSON()),
    attempt(() => a.desc.mapDesc(c.desc, true).toJSON()),
  ];
}

/** What src/bin/oracle/changes.rs answers for a desc read back from JSON. */
function jsonDesc(json: unknown) {
  const desc = ChangeDesc.fromJSON(json);
  const mapped =
    desc.length <= 60 ? Array.from({ length: desc.length + 1 }, (_, pos) => [desc.mapPos(pos, -1), desc.mapPos(pos, 1)]) : [];
  return [desc.toJSON(), desc.length, desc.newLength, mapped];
}

const changesJson = (set: ChangeSet, individual: boolean) => {
  const out: unknown[] = [];
  set.iterChanges((fa, ta, fb, tb, text) => out.push([fa, ta, fb, tb, text.toString()]), individual);
  return out;
};

function makeCase(rng: Rng) {
  const raw = rng.chance(0.03) ? source(rng, rng.logInt(200, 3000), 0.05) : source(rng, rng.int(40), rng.pick([0, 0.1, 0.3]));
  const doc = textOf(raw);
  const aSpecs = specs(rng, doc);
  const a = ChangeSet.of(toCm(aSpecs, doc.length), doc.length);
  const applied = a.apply(doc);
  const bSpecs = specs(rng, applied);
  const b = ChangeSet.of(toCm(bSpecs, applied.length), applied.length);
  const cSpecs = specs(rng, doc);
  const c = ChangeSet.of(toCm(cSpecs, doc.length), doc.length);

  const positions =
    doc.length <= 200 ? Array.from({ length: doc.length + 1 }, (_, i) => i) : Array.from({ length: 200 }, () => rng.int(doc.length + 1));
  const ranges = Array.from({ length: 6 }, () => {
    const from = rng.int(doc.length + 1);
    return [from, Math.min(doc.length, from + rng.int(5))] as [number, number];
  });
  const selections = Array.from({ length: rng.pick([0, 1, 2]) }, () => randomSelection(rng, doc));
  const selectionOps = selections.map((s) => {
    const [from, to] = [boundary(rng, doc), boundary(rng, doc)].sort((x, y) => x - y);
    return {
      extend: [from, to, rng.chance(0.3) ? rng.pick([-1, 1]) : 0] as [number, number, number],
      add: [randomRange(rng, doc), rng.chance(0.5)] as [RangeSpec, boolean],
      replace: [randomRange(rng, doc), rng.int(cmSelection(s).ranges.length)] as [RangeSpec, number],
    };
  });

  const inverted = a.invert(doc);
  const composed = a.compose(b);
  // Through JSON text, as a request carries them.
  const rawSet = JSON.parse(JSON.stringify(rawSetJson(rng, doc)));
  const rawSets = {
    a: rawSet,
    b: JSON.parse(JSON.stringify(rawSetJson(rng, ChangeSet.fromJSON(rawSet).apply(doc)))),
    c: JSON.parse(JSON.stringify(rawSetJson(rng, doc))),
  };
  const [docS, appliedS] = [doc.toString(), applied.toString()];
  const jsonSets: [unknown, string][] = [
    [a.toJSON(), docS],
    [b.toJSON(), appliedS],
    [c.toJSON(), docS],
    [inverted.toJSON(), appliedS],
    [composed.toJSON(), docS],
    [a.map(c).toJSON(), c.apply(doc).toString()],
    [rawSet, docS],
  ];
  const jsonDescs = [
    a.desc.toJSON(),
    a.invertedDesc.toJSON(),
    a.desc.mapDesc(c.desc, true).toJSON(),
    ChangeSet.fromJSON(rawSet).desc.toJSON(),
  ];
  const expected = {
    a: a.toJSON(),
    b: b.toJSON(),
    c: c.toJSON(),
    empty: a.empty,
    length: a.length,
    new_length: a.newLength,
    applied: applied.toString(),
    inverted: inverted.toJSON(),
    inverted_applied: inverted.apply(applied).toString(),
    inverted_desc: a.invertedDesc.toJSON(),
    composed: composed.toJSON(),
    composed_applied: composed.apply(doc).toString(),
    compose_desc: a.desc.composeDesc(b.desc).toJSON(),
    map: a.map(c).toJSON(),
    map_before: a.map(c, true).toJSON(),
    c_map_a: c.map(a).toJSON(),
    set_map_desc: a.mapDesc(c, true).toJSON(),
    map_desc: a.desc.mapDesc(c.desc).toJSON(),
    map_desc_before: a.desc.mapDesc(c.desc, true).toJSON(),
    map_pos: positions.map((pos) => [-1, 1].flatMap((assoc) => MODES.map((mode) => a.mapPos(pos, assoc, mode)))),
    changes: changesJson(a, false),
    changes_individual: changesJson(a, true),
    gaps: (() => {
      const out: unknown[] = [];
      a.iterGaps((x, y, l) => out.push([x, y, l]));
      return out;
    })(),
    changed_ranges: (() => {
      const out: unknown[] = [];
      a.desc.iterChangedRanges((fa, ta, fb, tb) => out.push([fa, ta, fb, tb]), true);
      return out;
    })(),
    touches: ranges.map(([f, t]) => a.touchesRange(f, t)),
    selections: selections.map((s, k) => {
      const sel = cmSelection(s);
      const ops = selectionOps[k];
      return [
        selectionJson(sel),
        selectionJson(sel.map(a)),
        selectionJson(sel.map(a, 1)),
        rangeJson(sel.main.extend(ops.extend[0], ops.extend[1], ops.extend[2])),
        selectionJson(sel.asSingle()),
        selectionJson(sel.addRange(cmRange(ops.add[0]), ops.add[1])),
        selectionJson(sel.replaceRange(cmRange(ops.replace[0]), ops.replace[1])),
        RECIPES.map((recipe) => byRange(doc, sel, recipe)),
      ];
    }),
    json_sets: jsonSets.map(jsonSet),
    json_descs: jsonDescs.map(jsonDesc),
    raw_ops: rawOps(rawSets),
  };
  const request = {
    op: "changes",
    doc: raw,
    a: aSpecs,
    b: bSpecs,
    c: cSpecs,
    positions,
    ranges,
    selections,
    selection_ops: selectionOps,
    json_sets: jsonSets,
    json_descs: jsonDescs,
    raw_sets: rawSets,
  };
  return { request, expected };
}

if (import.meta.main) {
  const binary = buildOracle();
  const checker = new Checker("changes oracle", binary, args.seed);
  let count = 0;
  for (let i = 0; i < args.cases; i++) {
    if (args.only !== null && i !== args.only) continue;
    const { request, expected } = makeCase(caseRng(args.seed, i));
    checker.add(`random #${i}`, `${script} ${args.cases} ${args.seed} ${i}`, request, expected);
    count++;
  }
  checker.finish(`${count} random cases`, count);
}
