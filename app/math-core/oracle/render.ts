// Display oracle: renders the same formulas with KaTeX JS (the app's `katex`)
// and with the Rust fork, under each app call site's options (sets.ts), and
// diffs the answers. Run from app/:
//
//   bun math-core/oracle/render.ts [--engine native|wasm] [--notes DIR]
//       [--katex DIR] [--out FILE] [--only a,b,…] [--no-build] [--oracle BIN]
//       [--pkg DIR] [--prefixes] [--source-map] [--stops]
//
// --engine native (default) runs the fork as the `oracle` bin; wasm runs the
//          app's build in math-core/pkg (built first by scripts/build-math.mjs)
//          and also checks its thrown errors' shape and that `parseError`
//          agrees with `renderToString`.
// --pkg    another wasm build directory (implies --no-build).
// --notes  where the user's .md files live (default: the app's data dir).
//          Read at run time only; formulas never leave the report file.
// --katex  a KaTeX checkout at the commit the fork tracks, for its spec
//          inputs (test/*.ts r`…` literals, screenshotter ss_data.yaml);
//          default math-core/target/katex when it exists (README.md).
//          The vendored tests/fixtures/upstream.json and our synthetic
//          fixtures.json (one formula per difference class) are always read.
// --oracle another build of the `oracle` bin to compare (implies --no-build).
// --out    the JSON report with every difference (default: $TMPDIR). It
//          holds the user's formulas, so it must stay outside the repo.
// --prefixes  the typing probe instead: the native bin renders every prefix
//          of every formula as the parse gate (set b) would and reports the
//          panics (report default: $TMPDIR/katex-prefix-report.json); with
//          --source-map, with `sourceMap` on.
// --source-map  the source-map property checks instead (sourcemap.ts): the
//          fork (either engine) renders every formula as the parse gate (set
//          b) would, with `sourceMap` off and on, and counts the failures per
//          check and kind (report default: $TMPDIR/katex-sourcemap-report.json).
// --stops  the edit field's corpus check instead: the native bin lays out
//          every formula's caret stops (oculus_math_edit) and checks their
//          invariants, and times the parse (report default:
//          $TMPDIR/katex-stops-report.json).
//
// Exit status 1 when any (formula, option set) pair differs, other than the
// accepted divergences below (DIVERGENCES.md), when a wasm check fails, when
// the probe finds a panic, or when a source-map check fails.

import { existsSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { type Formula, type Source, corpus } from "./corpus";
import {
  type Answer,
  type WasmChecks,
  checkStops,
  probePrefixes,
  renderFork,
  renderJs,
  withSourceMap,
} from "./engines";
import { type Failure, checkFormula } from "./sourcemap";
import { SETS, type Step } from "./sets";

const HERE = import.meta.dir;
const CORE = resolve(HERE, "..");

const argv = process.argv.slice(2);
function arg(name: string): string | undefined {
  const i = argv.indexOf(`--${name}`);
  return i >= 0 ? argv[i + 1] : undefined;
}
const notesDir = arg("notes") ?? join(homedir(), "Library/Application Support/com.tchan.oculus");
/** Where README.md's commands clone KaTeX (gitignored `target/`). */
const KATEX_CHECKOUT = join(CORE, "target/katex");
const katexDir = arg("katex") ?? (existsSync(join(KATEX_CHECKOUT, "test")) ? KATEX_CHECKOUT : undefined);
const probe = argv.includes("--prefixes");
const sourceMap = argv.includes("--source-map");
const stopCheck = argv.includes("--stops");
const reportName = probe
  ? "katex-prefix-report.json"
  : stopCheck
    ? "katex-stops-report.json"
    : sourceMap
      ? "katex-sourcemap-report.json"
      : "katex-oracle-report.json";
const outFile = arg("out") ?? join(tmpdir(), reportName);
const only = arg("only")?.split(",");
const engineArg = probe || stopCheck ? "native" : (arg("engine") ?? "native");
if (engineArg !== "native" && engineArg !== "wasm") throw new Error(`--engine ${engineArg}: native or wasm`);
const engine: "native" | "wasm" = engineArg;
const oracleBin = arg("oracle");
const pkgDir = arg("pkg");
const build = !argv.includes("--no-build") && !(engine === "native" ? oracleBin : pkgDir);
const bin = oracleBin ?? join(CORE, "target/release/oracle");
const pkg = pkgDir ?? join(CORE, "pkg");

type Verdict = "equal" | "accepted" | "html-differs" | "error-mismatch" | "error-text-differs" | "panic";

/** DIVERGENCES.md's accepted differences: rewriting KaTeX JS's output this
 *  way gives the fork's. */
const ACCEPTED: { id: string; js: RegExp; fork: string }[] = [
  // D1: KaTeX JS writes `undefined` for \overlinesegment/\underlinesegment's
  // MathML operator (no stretchy code point); the fork writes a space.
  { id: "D1", js: /<mo stretchy="true">undefined<\/mo>/g, fork: '<mo stretchy="true"> </mo>' },
];

function acceptedOnly(js: string, fork: string): boolean {
  let out = js;
  for (const a of ACCEPTED) out = out.replace(a.js, a.fork);
  return out !== js && out === fork;
}

function outcome(answers: Answer[]): Answer {
  // A call site's result is its last step's: every earlier one threw.
  for (const a of answers) if (!("error" in a)) return a;
  return answers[answers.length - 1];
}

function verdict(js: Answer, rs: Answer): Verdict {
  if ("panic" in rs) return "panic";
  if ("html" in js && "html" in rs) {
    if (js.html === rs.html) return "equal";
    return acceptedOnly(js.html, rs.html) ? "accepted" : "html-differs";
  }
  if ("error" in js && "error" in rs) return js.error === rs.error ? "equal" : "error-text-differs";
  return "error-mismatch";
}

/** Where two strings first differ, digits masked, as a grouping key. */
function signature(a: string, b: string): string {
  let i = 0;
  while (i < a.length && i < b.length && a[i] === b[i]) i++;
  const cut = (s: string) => s.slice(Math.max(0, i - 24), i + 24).replace(/\d+(\.\d+)?/g, "#");
  return `${cut(a)}  ≠  ${cut(b)}`;
}

function buildEngine() {
  if (!build) return;
  const cmd =
    engine === "native"
      ? ["cargo", "build", "--release", "--bin", "oracle"]
      : ["node", join(CORE, "../scripts/build-math.mjs")];
  const done = Bun.spawnSync(cmd, { cwd: CORE, stdout: "inherit", stderr: "inherit" });
  if (done.exitCode !== 0) process.exit(2);
}

/** The typing probe: set b's options on every prefix, panics only; with
 *  --source-map, as the edit field renders them. */
async function prefixes(formulas: Formula[]) {
  const gate = SETS.find((s) => s.name === "b")!;
  const steps = formulas.map((f) => (sourceMap ? withSourceMap(gate.steps(f)[0]) : gate.steps(f)[0]));
  const t0 = performance.now();
  const { prefixes: count, panics } = await probePrefixes(steps, bin);
  console.log(`prefixes: ${count} rendered in ${((performance.now() - t0) / 1000).toFixed(1)} s; ${panics.length} panics`);
  const byMessage = new Map<string, number>();
  for (const p of panics) byMessage.set(p.panic, (byMessage.get(p.panic) ?? 0) + 1);
  for (const [message, n] of [...byMessage].sort((x, y) => y[1] - x[1])) console.log(`${String(n).padStart(6)}  ${message}`);
  const report = panics.map((p) => ({ ...p, source: formulas[p.step].source, prefix: steps[p.step].tex.slice(0, p.len) }));
  writeFileSync(outFile, JSON.stringify(report, null, 1));
  console.log(`\nreport: ${outFile}`);
  process.exit(panics.length ? 1 : 0);
}

/** The source-map property checks: set b's options, `sourceMap` off and on. */
async function sourceMapChecks(formulas: Formula[]) {
  const gate = SETS.find((s) => s.name === "b")!;
  const off = formulas.map((f) => gate.steps(f)[0]);
  const t0 = performance.now();
  const { answers, checks } = await renderFork([...off, ...off.map(withSourceMap)], engine, { bin, pkg });
  console.log(`source map: ${formulas.length} formulas rendered off and on in ${((performance.now() - t0) / 1000).toFixed(1)} s (${engine})`);
  if (checks) {
    console.log(`wasm: ${checks.errorShape} badly shaped throws, ${checks.gate} parseError disagreements, ${checks.traps} traps`);
  }

  const names: Record<Failure["check"], string> = {
    a: "errors unchanged",
    b: "ranges well-formed and nested",
    c: "every glyph mapped",
    d: "source covered by leaves",
    e: "flag-off identity",
  };
  const failing: Record<string, number> = { a: 0, b: 0, c: 0, d: 0, e: 0 };
  const groups = new Map<string, { check: string; formulas: number; examples: { tex: string; source: Source }[] }>();
  const identity: Record<string, number> = {};
  const excluded = new Map<string, number>();
  let errorsMoved = 0;
  const report: { tex: string; source: Source; failures: Failure[]; identity: string }[] = [];
  formulas.forEach((f, i) => {
    const out = checkFormula(f.tex, answers[i], answers[formulas.length + i]);
    identity[out.identity] = (identity[out.identity] ?? 0) + 1;
    if (out.errorMoved) errorsMoved++;
    for (const [r, n] of out.excluded) excluded.set(r, (excluded.get(r) ?? 0) + n);
    if (!out.failures.length) return;
    report.push({ tex: f.tex, source: f.source, failures: out.failures, identity: out.identity });
    for (const c of new Set(out.failures.map((x) => x.check))) failing[c]++;
    for (const key of new Set(out.failures.map((x) => `${x.check}  ${x.key}`))) {
      const g = groups.get(key) ?? { check: key[0], formulas: 0, examples: [] };
      g.formulas++;
      g.examples.push({ tex: f.tex, source: f.source });
      groups.set(key, g);
    }
  });

  console.log("\ncheck                                  formulas failing");
  for (const c of Object.keys(names) as Failure["check"][]) {
    console.log(`${c}  ${names[c].padEnd(36)} ${String(failing[c]).padStart(6)}`);
  }
  console.log(`\nerrors at another position only (a, not failures): ${errorsMoved}`);
  console.log(`identity (e): ${Object.entries(identity).map(([k, n]) => `${n} ${k}`).join(", ")}`);
  console.log("\nexcluded from coverage (characters):");
  for (const [r, n] of [...excluded].sort((x, y) => y[1] - x[1])) console.log(`${String(n).padStart(8)}  ${r}`);

  console.log("\nfailures by kind (formulas; shortest example, fixtures and spec first):");
  const sorted = [...groups].sort((x, y) => x[1].check.localeCompare(y[1].check) || y[1].formulas - x[1].formulas);
  for (const [key, g] of sorted.slice(0, 80)) {
    const rank = (s: Source) => (s === "notes" ? 1 : 0);
    const ex = g.examples.sort((x, y) => rank(x.source) - rank(y.source) || x.tex.length - y.tex.length)[0];
    console.log(`${String(g.formulas).padStart(6)}  ${key}    e.g. [${ex.source}] ${ex.tex.length > 70 ? `${ex.tex.slice(0, 70)}…` : ex.tex}`);
  }
  if (sorted.length > 80) console.log(`  … ${sorted.length - 80} more kinds in the report`);

  writeFileSync(outFile, JSON.stringify({ failing, identity, excluded: Object.fromEntries(excluded), formulas: report }, null, 1));
  console.log(`\nreport: ${outFile}`);
  const failed = report.length > 0 || (checks && (checks.errorShape || checks.gate || checks.traps));
  process.exit(failed ? 1 : 0);
}

/** The edit field's corpus check: counts only, and the first few failing
 *  formulas cut short (the notes are the user's). */
async function stops(formulas: Formula[]) {
  const t0 = performance.now();
  const answers = await checkStops(formulas, bin);
  const count = (s: Source) => formulas.filter((f) => f.source === s).length;
  console.log(
    `stops: ${formulas.length} formulas (${count("notes")} notes, ${count("spec")} spec, ${count("fixture")} fixtures) checked in ${((performance.now() - t0) / 1000).toFixed(1)} s`,
  );
  const failing: { tex: string; source: Source; failures: { kind: string; offset: number }[] }[] = [];
  for (const [title, keep] of [
    ["all inputs", (_: Formula) => true],
    ["notes only", (f: Formula) => f.source === "notes"],
  ] as const) {
    let rendered = 0;
    let unrendered = 0;
    let panics = 0;
    let stopCount = 0;
    let slots = 0;
    let between = 0;
    const kinds = new Map<string, number>();
    const errors = new Map<string, number>();
    formulas.forEach((f, i) => {
      if (!keep(f)) return;
      const a = answers[i];
      if ("panic" in a) panics++;
      else if ("error" in a) {
        unrendered++;
        // The message's kind, without the name, position and context it quotes.
        const kind = a.error.replace(/^KaTeX parse error: /, "").split(/:| at position| '|\\/)[0].trim();
        errors.set(kind, (errors.get(kind) ?? 0) + 1);
      } else {
        rendered++;
        stopCount += a.stops;
        slots += a.slots;
        between += a.between;
        for (const kind of new Set(a.failures.map((x) => x.kind))) kinds.set(kind, (kinds.get(kind) ?? 0) + 1);
      }
    });
    console.log(`\n${title}: ${rendered} with stops (${stopCount} stops, ${slots} slots, ${between} stops between two others at one offset)`);
    console.log(`  ${unrendered} do not render (no stops: TeX mode), ${panics} panics`);
    const byKind = [...errors].sort((x, y) => y[1] - x[1]).map(([k, n]) => `${n} ${k}`);
    if (byKind.length) console.log(`    by error: ${byKind.slice(0, 8).join("; ")}`);
    console.log(`  failing formulas by kind: ${kinds.size ? [...kinds].map(([k, n]) => `${n} ${k}`).join(", ") : "none"}`);
  }
  formulas.forEach((f, i) => {
    const a = answers[i];
    if ("panic" in a) failing.push({ tex: f.tex, source: f.source, failures: [{ kind: `panic: ${a.panic}`, offset: 0 }] });
    else if ("failures" in a && a.failures.length) failing.push({ tex: f.tex, source: f.source, failures: a.failures });
  });
  if (failing.length) {
    console.log("\nfirst failing formulas (fixtures and spec first):");
    const rank = (s: Source) => (s === "notes" ? 1 : 0);
    const shown = [...failing].sort((x, y) => rank(x.source) - rank(y.source) || x.tex.length - y.tex.length).slice(0, 5);
    for (const f of shown) {
      const tex = f.tex.length > 60 ? `${f.tex.slice(0, 60)}…` : f.tex;
      console.log(`  [${f.source}] ${tex}  ${f.failures.map((x) => `${x.kind} @${x.offset}`).slice(0, 3).join("; ")}`);
    }
  }

  const timing = (title: string, keep: (f: Formula) => boolean) => {
    const ns = formulas.flatMap((f, i) => (keep(f) && "parseNs" in answers[i] ? [answers[i].parseNs as number] : []));
    if (!ns.length) return;
    ns.sort((x, y) => x - y);
    const mean = ns.reduce((x, y) => x + y, 0) / ns.length;
    const at = (q: number) => ns[Math.min(ns.length - 1, Math.floor(q * ns.length))];
    const us = (n: number) => `${(n / 1000).toFixed(1)} µs`;
    console.log(`  ${title.padEnd(10)} ${String(ns.length).padStart(5)} formulas  mean ${us(mean)}  p50 ${us(at(0.5))}  p99 ${us(at(0.99))}  max ${us(ns[ns.length - 1])}`);
  };
  console.log("\nparse with source mapping (release build, mean of 5 per formula):");
  timing("all", () => true);
  timing("fixtures", (f) => f.source === "fixture");
  timing("notes", (f) => f.source === "notes");
  timing("spec", (f) => f.source === "spec");

  writeFileSync(outFile, JSON.stringify(failing, null, 1));
  console.log(`\nreport: ${outFile}`);
  process.exit(failing.length ? 1 : 0);
}

async function main() {
  buildEngine();
  const formulas = corpus(notesDir, katexDir);
  if (probe) return prefixes(formulas);
  if (stopCheck) return stops(formulas);
  if (sourceMap) return sourceMapChecks(formulas);
  const sets = SETS.filter((s) => !only || only.includes(s.name));
  const count = (s: Source) => formulas.filter((f) => f.source === s).length;
  console.log(
    `corpus: ${formulas.length} unique (${count("notes")} notes, ${count("spec")} spec, ${count("fixture")} fixtures)`,
  );

  // Every step of every set, deduplicated: sets share many renders.
  const keyOf = (s: Step) => JSON.stringify([s.tex, s.options]);
  const unique = new Map<string, Step>();
  const plan = formulas.map((f) =>
    sets.map((set) =>
      set.steps(f).map((s) => {
        const k = keyOf(s);
        if (!unique.has(k)) unique.set(k, s);
        return k;
      }),
    ),
  );
  const steps = [...unique.values()];
  const warn = console.warn;
  console.warn = () => {}; // strict: "warn" (KaTeX's default) logs per formula
  const t0 = performance.now();
  const js = new Map(steps.map((s) => [keyOf(s), renderJs(s)]));
  const t1 = performance.now();
  console.warn = warn;
  let rsList: Answer[];
  let checks: WasmChecks | undefined;
  ({ answers: rsList, checks } = await renderFork(steps, engine, { bin, pkg }));
  const t2 = performance.now();
  const rs = new Map(steps.map((s, i) => [keyOf(s), rsList[i]]));
  const label = engine === "wasm" ? "fork (wasm, incl. gate checks)" : "fork (incl. spawn)";
  console.log(`renders: ${steps.length} unique; KaTeX JS ${(t1 - t0).toFixed(0)} ms, ${label} ${(t2 - t1).toFixed(0)} ms`);
  if (checks) {
    console.log(`wasm: ${checks.errorShape} badly shaped throws, ${checks.gate} parseError disagreements, ${checks.traps} traps`);
  }

  const rows: Record<string, Record<Verdict, number>> = {};
  const fixtureClasses = new Map<string, Set<string>>(); // class → sets that differ
  const diffs: { set: string; verdict: Verdict; source: Source; display: boolean; tex: string; js: Answer; rs: Answer; sig: string }[] = [];
  formulas.forEach((f, fi) => {
    sets.forEach((set, si) => {
      // Run the steps as the call site would: stop at the first that succeeds.
      const keys = plan[fi][si];
      const jsAns: Answer[] = [];
      const rsAns: Answer[] = [];
      for (const k of keys) {
        jsAns.push(js.get(k)!);
        if (!("error" in js.get(k)!)) break;
      }
      for (const k of keys) {
        rsAns.push(rs.get(k)!);
        if (!("error" in rs.get(k)!)) break;
      }
      const a = outcome(jsAns);
      const b = outcome(rsAns);
      let v = verdict(a, b);
      // rehype-katex's strict render failing is a step, not the outcome; but
      // one engine accepting what the other rejects there is still a mismatch.
      if ((v === "equal" || v === "accepted") && jsAns.length !== rsAns.length) v = "error-mismatch";
      for (const key of [set.name, `${set.name}/${f.source}`]) {
        const row = (rows[key] ??= { equal: 0, accepted: 0, "html-differs": 0, "error-mismatch": 0, "error-text-differs": 0, panic: 0 });
        row[v]++;
      }
      if (f.class) {
        const differing = fixtureClasses.get(f.class) ?? new Set<string>();
        if (v !== "equal") differing.add(v === "accepted" ? `${set.name}(accepted)` : set.name);
        fixtureClasses.set(f.class, differing);
      }
      if (v !== "equal" && v !== "accepted") {
        const sig =
          "html" in a && "html" in b
            ? signature(a.html, b.html)
            : "error" in a && "error" in b
              ? signature(a.error, b.error)
              : `${"error" in jsAns[0] ? "JS throws" : "JS renders"} / ${"error" in rsAns[0] ? "fork throws" : "fork renders"}`;
        diffs.push({ set: set.name, verdict: v, source: f.source, display: f.display, tex: f.tex, js: a, rs: b, sig });
      }
    });
  });

  for (const [title, suffix] of [["all inputs", ""], ["notes only", "/notes"]]) {
    console.log(`\n${title}\nset  call site                                equal  accepted  html-differs  error-mismatch  error-text-differs  panic`);
    for (const set of sets) {
      const r = rows[set.name + suffix];
      if (!r) continue;
      console.log(
        `${set.name.padEnd(4)} ${set.where.padEnd(40)} ${String(r.equal).padStart(5)}  ${String(r.accepted).padStart(8)}  ${String(r["html-differs"]).padStart(12)}  ${String(r["error-mismatch"]).padStart(14)}  ${String(r["error-text-differs"]).padStart(18)}  ${String(r.panic).padStart(5)}`,
      );
    }
  }

  console.log("\nfixtures.json classes (sets still differing):");
  for (const [c, differing] of fixtureClasses) console.log(`  ${c.padEnd(20)} ${differing.size ? [...differing].join(" ") : "equal"}`);

  // Difference classes, by first-difference signature, across sets.
  const classes = new Map<string, { count: number; sets: Set<string>; verdict: Verdict }>();
  for (const d of diffs) {
    const c = classes.get(d.sig) ?? { count: 0, sets: new Set(), verdict: d.verdict };
    c.count++;
    c.sets.add(d.set);
    classes.set(d.sig, c);
  }
  const sorted = [...classes.entries()].sort((x, y) => y[1].count - x[1].count);
  console.log(`\n${sorted.length} difference classes (by first differing bytes, digits masked):`);
  for (const [sig, c] of sorted.slice(0, 40)) console.log(`${String(c.count).padStart(5)}  [${[...c.sets].join("")}] ${c.verdict}: ${sig}`);

  writeFileSync(outFile, JSON.stringify({ rows, classes: sorted.map(([sig, c]) => ({ sig, ...c, sets: [...c.sets] })), diffs }, null, 1));
  console.log(`\nreport: ${outFile}`);
  const failed = diffs.length > 0 || (checks && (checks.errorShape || checks.gate || checks.traps));
  process.exit(failed ? 1 : 0);
}

if (import.meta.main) await main();
