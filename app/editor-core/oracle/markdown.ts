// Checks the Rust markdown parser against the app's own @lezer/markdown
// configuration (`editor/core/language.ts`'s `noteLanguage`). Each case is one
// document, parsed through CodeMirror's `DocInput` as the editor parses it;
// the tree is compared as `name from to` in pre-order with nested code
// languages pruned (IgnoreMounts), and `resolveInner(pos, side)` probes are
// compared as ancestor chains; documents up to NAV_LIMIT units also compare
// every node's parent/children/siblings and `resolveInner` at every position.
// Navigation runs on the same configuration minus `CodeLanguages` (whose
// overlays `resolveInner` would enter); each case first checks that both
// configurations give the same pruned tree.
//
// Documents: line-structured markdown built from constructs that stress the
// grammar (nested containers with lazy lines, delimiter runs, link refs,
// maths, frontmatter, unclosed fences, HTML, entities, Thai, emoji, CRLF),
// tables, mutations of those, soups of markup characters, constructs at
// their unit-counted limits, plus the app's notes (generators in
// `markdown-docs.ts`).
//
//   bun editor-core/oracle/markdown.ts [cases] [seed] [only]     (from app/)

import { DocInput } from "@codemirror/language";
import { Text } from "@codemirror/state";
import { IterMode, type Tree } from "@lezer/common";
import { Autolink, type MarkdownParser, Strikethrough, Table, TaskList } from "@lezer/markdown";
import { commonmarkLanguage } from "@codemirror/lang-markdown";

import { FrontmatterSyntax } from "../../src/components/documents/editor/syntax/frontmatter";
import { noteLanguage } from "../../src/components/documents/editor/core/language";
import { MathSyntax } from "../../src/components/documents/editor/math/mathSyntax";
import { Checker, type Rng, buildOracle, caseRng, corpus, parseArgs } from "./driver";
import { edges, editSteps, mutated, probes, soup, structured, table } from "./markdown-docs";

const SPLIT = /\r\n?|\n/; // CodeMirror's DefaultSplit
/** Documents up to this many units get the full navigation check. */
const NAV_LIMIT = 2000;
/** Documents up to this many units may get an edit sequence, reparsed
 *  incrementally on the Rust side and freshly by Lezer. */
const EDIT_LIMIT = 5000;

const args = parseArgs(20000);
const script = "bun editor-core/oracle/markdown.ts";

const appParser = noteLanguage.parser as MarkdownParser;
const plainParser = (commonmarkLanguage.parser as MarkdownParser).configure([
  Table,
  Strikethrough,
  TaskList,
  Autolink,
  MathSyntax,
  FrontmatterSyntax,
]);

function dump(tree: Tree): string[] {
  const out: string[] = [];
  tree.iterate({
    mode: IterMode.IgnoreMounts,
    enter: (n) => {
      out.push(`${n.name} ${n.from} ${n.to}`);
    },
  });
  return out;
}

type SyntaxNode = ReturnType<Tree["resolveInner"]>;

const key = (n: SyntaxNode | null) => (n ? `${n.name} ${n.from} ${n.to}` : "-");

function chain(tree: Tree, pos: number, side: -1 | 0 | 1): string[] {
  const out: string[] = [];
  for (let n: SyntaxNode | null = tree.resolveInner(pos, side); n; n = n.parent) out.push(key(n));
  return out;
}

/** Per node in pre-order: parent, first child, last child, next and
 *  previous sibling. */
function nav(tree: Tree): string[] {
  const out: string[] = [];
  tree.iterate({
    mode: IterMode.IgnoreMounts,
    enter: (r) => {
      const n = r.node;
      out.push([n.parent, n.firstChild, n.lastChild, n.nextSibling, n.prevSibling].map(key).join("|"));
    },
  });
  return out;
}

/** `resolveInner` at every position 0..=len+1 and side -1, 0, 1. */
function allPositions(tree: Tree, len: number): string[] {
  const out: string[] = [];
  for (let p = 0; p <= len + 1; p++) for (const s of [-1, 0, 1] as const) out.push(chain(tree, p, s).join("<"));
  return out;
}

const binary = buildOracle();
const checker = new Checker("markdown oracle", binary, args.seed);
let configMismatches = 0;
let nodeCount = 0;
let navigated = 0;
let editSequences = 0;
const seen = new Set<string>();

function check(label: string, replay: string, raw: string, rng: Rng, nodeTypes = false) {
  const doc = Text.of(raw.split(SPLIT));
  const tree = appParser.parse(new DocInput(doc));
  const nodes = dump(tree);
  nodeCount += nodes.length;
  for (const n of nodes) seen.add(n.slice(0, n.indexOf(" ")));
  const plain = plainParser.parse(new DocInput(doc));
  const plainNodes = dump(plain);
  if (plainNodes.join("\n") !== nodes.join("\n")) {
    configMismatches++;
    if (configMismatches <= 3) console.error(`CONFIG MISMATCH ${label}: pruned trees differ without CodeLanguages`);
  }
  const ps = probes(rng, doc.length, nodes);
  const expected: Record<string, unknown> = { nodes, resolved: ps.map(([p, s]) => chain(plain, p, s)) };
  const request: Record<string, unknown> = { op: "markdown", text: raw, probes: ps };
  if (doc.length <= NAV_LIMIT) {
    navigated++;
    request.nav = request.all_positions = true;
    expected.nav = nav(plain);
    expected.all_positions = allPositions(plain, doc.length);
  }
  if (doc.length <= EDIT_LIMIT && rng.chance(0.3)) {
    editSequences++;
    const { steps, docs } = editSteps(rng, doc);
    request.edits = steps;
    request.edit_nav_limit = NAV_LIMIT;
    expected.edits = docs.map((d) => {
      const fresh = plainParser.parse(new DocInput(d));
      const step: Record<string, unknown> = { nodes: dump(appParser.parse(new DocInput(d))), nav: nav(fresh) };
      if (d.length <= NAV_LIMIT) step.all_positions = allPositions(fresh, d.length);
      return step;
    });
  }
  if (nodeTypes) {
    request.node_types = true;
    expected.node_types = appParser.nodeSet.types.map((t) => t.name);
  }
  checker.add(label, replay, request, expected);
}

const kinds = { structured: 0, mutated: 0, table: 0, soup: 0, edges: 0 };
for (let i = 0; i < args.cases; i++) {
  if (args.only !== null && i !== args.only) continue;
  const rng = caseRng(args.seed, i);
  const r = rng.next();
  const kind = r < 0.4 ? "structured" : r < 0.6 ? "mutated" : r < 0.75 ? "table" : r < 0.95 ? "soup" : "edges";
  kinds[kind]++;
  const make = { structured, mutated, table, soup, edges }[kind];
  const raw = make(rng);
  // A replayed case is synthetic, so print it whole.
  if (args.only !== null) console.error(`doc: ${JSON.stringify(raw)}`);
  check(`random #${i} (${kind})`, `${script} ${args.cases} ${args.seed} ${i}`, raw, rng, i === 0);
}

let notes = 0;
if (args.only === null) {
  for (const { name, source } of await corpus()) {
    notes++;
    check(`note ${name}`, `${script} 0 ${args.seed}`, source, caseRng(args.seed, -1 - notes));
  }
}

const unseen = appParser.nodeSet.types.filter((t) => t.id > 0 && !seen.has(t.name)).map((t) => t.name);
const summary =
  `${Object.entries(kinds).map(([k, n]) => `${n} ${k}`).join(" + ")} docs ` +
  `(${navigated} fully navigated, ${editSequences} edited), ` +
  `${notes} notes, ${nodeCount} nodes` +
  (unseen.length ? ` (types never produced: ${unseen.join(", ")})` : ", every node type produced");
if (configMismatches) {
  checker.flush();
  console.error(`markdown oracle: ${summary}, ${configMismatches} CONFIG MISMATCHES (seed ${args.seed})`);
  process.exit(1);
}
checker.finish(summary, Object.values(kinds).reduce((a, b) => a + b, 0));
