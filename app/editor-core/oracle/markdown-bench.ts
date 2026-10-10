// Times @lezer/markdown (the app's configuration) on benches/synthetic-note.md
// — a full parse and one-keystroke reparses with `TreeFragment` reuse, the
// way CodeMirror reparses, in the same places as `cargo bench --bench
// markdown`. With `--write` it first (re)writes the note, deterministically.
//
//   bun editor-core/oracle/markdown-bench.ts [--write]      (from app/)

import { join } from "node:path";

import { DocInput } from "@codemirror/language";
import { ChangeSet, Text } from "@codemirror/state";
import { TreeFragment } from "@lezer/common";

import { noteLanguage } from "../../src/components/documents/editor/core/language";
import { Rng, crate } from "./driver";

const path = join(crate, "benches/synthetic-note.md");

/** A long lecture note: headings, prose with inline marks, lists, tasks,
 *  quotes, tables, maths, code. */
function note(): string {
  const rng = new Rng(7);
  const words = ["the", "matrix", "eigenvalue", "ไทย", "proof", "lemma", "vector", "😀", "space", "field", "map"];
  const sentence = () => {
    const parts: string[] = [];
    for (let i = 4 + rng.int(12); i > 0; i--) {
      const w = rng.pick(words);
      parts.push(rng.chance(0.08) ? `*${w}*` : rng.chance(0.05) ? `**${w}**` : rng.chance(0.04) ? `\`${w}\`` : rng.chance(0.04) ? `$${w}^2$` : rng.chance(0.02) ? `[${w}](https://ex.com/${w})` : w);
    }
    return `${parts.join(" ")}.`;
  };
  const out: string[] = ["---", "title: Linear algebra", "tags: [maths, notes]", "---", ""];
  for (let section = 1; out.join("\n").length < 200_000; section++) {
    out.push(`## Section ${section}`, "");
    for (let p = 0; p < 4; p++) out.push(Array.from({ length: 2 + rng.int(4) }, sentence).join(" "), "");
    out.push(...Array.from({ length: 4 }, (_, i) => `${rng.chance(0.5) ? "-" : `${i + 1}.`} ${rng.chance(0.3) ? "[ ] " : ""}${sentence()}`), "");
    out.push(`> ${sentence()}`, `> ${sentence()}`, "");
    out.push("| a | b | c |", "|---|:-:|--:|", ...Array.from({ length: 3 }, () => `| ${rng.pick(words)} | $x_${rng.int(9)}$ | ${rng.int(100)} |`), "");
    out.push("$$", "\\int_0^1 f(x)\\,dx = F(1) - F(0)", "$$", "");
    out.push("```python", "def f(x):", "    return x * 2", "```", "");
  }
  return out.join("\n");
}

if (process.argv.includes("--write")) await Bun.write(path, note());
const doc = Text.of((await Bun.file(path).text()).split("\n"));
const parser = noteLanguage.parser;

function time(label: string, runs: number, f: () => void) {
  for (let i = 0; i < 20; i++) f();
  const start = performance.now();
  for (let i = 0; i < runs; i++) f();
  console.log(`${label}: ${(((performance.now() - start) / runs) * 1000).toFixed(1)} µs`);
}

/** After the first char of the first prose line at or after line `line`. */
function keystrokeAt(d: Text, line: number): number {
  while (!/^[a-z]/.test(d.line(line).text)) line++;
  return d.line(line).from + 1;
}

function keystroke(label: string, d: Text, at: number) {
  const tree = parser.parse(new DocInput(d));
  const next = ChangeSet.of({ from: at, insert: "x" }, d.length).apply(d);
  const fragments = TreeFragment.applyChanges(TreeFragment.addTree(tree), [{ fromA: at, toA: at, fromB: at, toB: at + 1 }]);
  time(label, 500, () => parser.parse(new DocInput(next), fragments));
}

console.log(`lezer on ${doc.length} units, ${doc.lines} lines`);
time("full parse", 50, () => parser.parse(new DocInput(doc)));
keystroke("keystroke, middle (past the window)", doc, keystrokeAt(doc, Math.floor(doc.lines / 2)));
keystroke("keystroke, inside the frontmatter window", doc, keystrokeAt(doc, doc.lineAt(1000).number));
// The lines before the one holding unit 30 000.
const short = doc.slice(0, doc.lineAt(30_000).from - 1);
keystroke("keystroke, middle of a 30 KB note", short, keystrokeAt(short, Math.floor(short.lines / 2)));
