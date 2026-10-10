// Display oracle: renders the same formulas with KaTeX JS (the app's `katex`)
// and with the Rust fork (`oracle` bin), under each app call site's options,
// and diffs the answers. Run from app/:
//
//   bun math-core/oracle/render.ts [--notes DIR] [--katex DIR] [--out FILE]
//                                  [--only a,b,…] [--no-build] [--oracle BIN]
//
// --notes  where the user's .md files live (default: the app's data dir).
//          Read at run time only; formulas never leave the report file.
// --katex  a KaTeX checkout at the commit the fork tracks, for its spec
//          inputs (test/*.ts r`…` literals, screenshotter ss_data.yaml).
//          The vendored tests/fixtures/upstream.json and our synthetic
//          fixtures.json (one formula per difference class) are always read.
// --oracle another build of the `oracle` bin to compare (implies --no-build).
// --out    the JSON report with every difference (default: $TMPDIR). It
//          holds the user's formulas, so it must stay outside the repo.
//
// Exit status 1 when any (formula, option set) pair differs, other than the
// accepted divergences below (DIVERGENCES.md).

import katex from "katex";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";

const HERE = import.meta.dir;
const CORE = resolve(HERE, "..");

// ── Arguments ───────────────────────────────────────────────────────────────

const argv = process.argv.slice(2);
function arg(name: string): string | undefined {
  const i = argv.indexOf(`--${name}`);
  return i >= 0 ? argv[i + 1] : undefined;
}
const notesDir = arg("notes") ?? join(homedir(), "Library/Application Support/com.tchan.oculus");
const katexDir = arg("katex");
const outFile = arg("out") ?? join(tmpdir(), "katex-oracle-report.json");
const only = arg("only")?.split(",");
const oracleBin = arg("oracle");
const build = !argv.includes("--no-build") && !oracleBin;

// ── Corpus ──────────────────────────────────────────────────────────────────

type Source = "fixture" | "notes" | "spec";
interface Formula {
  tex: string;
  display: boolean;
  source: Source;
  /** fixtures.json's difference class. */
  class?: string;
}

function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    const st = statSync(path);
    if (st.isDirectory()) walk(path, out);
    else if (name.endsWith(".md")) out.push(path);
  }
  return out;
}

/** `$$…$$` (display) and pandoc-style `$…$` (inline: no space just inside
 *  either `$`, closing `$` not followed by a digit) outside code. */
export function extractMaths(md: string): { tex: string; display: boolean }[] {
  const found: { tex: string; display: boolean }[] = [];
  // Fenced code blocks, then inline code spans, blanked out.
  let text = md.replace(/^( {0,3})(`{3,}|~{3,})[^\n]*\n[\s\S]*?(?:^\1?\2[`~]*[ \t]*$|(?![\s\S]))/gm, " ");
  text = text.replace(/(`+)(?!`)[\s\S]*?(?<!`)\1(?!`)/g, " ");
  text = text.replace(/(?<!\\)\$\$([\s\S]+?)\$\$/g, (_, tex: string) => {
    if (tex.trim()) found.push({ tex: tex.trim(), display: true });
    return " ";
  });
  const inline = /(?<![\\$])\$(?![\s$])((?:[^$\\\n]|\\.)+?)(?<![\s\\])\$(?!\d)/g;
  for (const m of text.matchAll(inline)) found.push({ tex: m[1], display: false });
  return found;
}

function specInputs(): Formula[] {
  const out: Formula[] = [];
  const fixture = JSON.parse(readFileSync(join(CORE, "katex/tests/fixtures/upstream.json"), "utf8"));
  for (const c of fixture.cases) out.push({ tex: c.expression, display: !!c.displayMode, source: "spec" });
  if (!katexDir) return out;
  const test = join(katexDir, "test");
  for (const name of readdirSync(test).filter((n) => n.endsWith("-spec.ts"))) {
    const src = readFileSync(join(test, name), "utf8");
    for (const m of src.matchAll(/\br`((?:[^`\\]|\\[\s\S])*)`/g)) {
      if (m[1].includes("${")) continue;
      out.push({ tex: m[1], display: false, source: "spec" }, { tex: m[1], display: true, source: "spec" });
    }
  }
  const ss = join(test, "screenshotter/ss_data.yaml");
  if (existsSync(ss)) {
    const data = Bun.YAML.parse(readFileSync(ss, "utf8")) as Record<string, string | { tex: string; display?: number }>;
    for (const v of Object.values(data)) {
      const tex = typeof v === "string" ? v : v.tex;
      const display = typeof v === "string" ? false : !!v.display;
      if (typeof tex === "string") out.push({ tex, display, source: "spec" });
    }
  }
  return out;
}

function corpus(): Formula[] {
  const seen = new Map<string, Formula>();
  const add = (f: Formula) => {
    const key = `${f.display ? "D" : "I"}${f.tex}`;
    if (!seen.has(key)) seen.set(key, f);
  };
  const fixtures = JSON.parse(readFileSync(join(HERE, "fixtures.json"), "utf8")) as Formula[];
  for (const f of fixtures) add({ ...f, source: "fixture" });
  if (existsSync(notesDir)) {
    for (const file of walk(notesDir)) {
      for (const m of extractMaths(readFileSync(file, "utf8"))) add({ ...m, source: "notes" });
    }
  }
  for (const f of specInputs()) add(f);
  return [...seen.values()];
}

// ── Option sets: the app's call sites ───────────────────────────────────────

/** widgets.ts's pre-pass: `\left[\begin{array}…\end{array}\right]` gets
 *  `\kern-0.5em` inside the brackets. A copy, kept in step by hand. */
const LEFT_BEFORE = /\\left\s*(?:\\[a-zA-Z]+|\\.|[^\s\\])\s*$/;
function hugArrays(source: string): string {
  const BEGIN = "\\begin{array}";
  const END = "\\end{array}";
  if (!source.includes(BEGIN)) return source;
  let out = "";
  let done = 0;
  for (let at = source.indexOf(BEGIN); at >= 0; at = source.indexOf(BEGIN, at + 1)) {
    if (at < done || !LEFT_BEFORE.test(source.slice(0, at))) continue;
    let depth = 0;
    let end = -1;
    for (let i = at; i < source.length; i++) {
      if (source.startsWith(BEGIN, i)) depth++;
      else if (source.startsWith(END, i) && --depth === 0) {
        end = i + END.length;
        break;
      }
    }
    if (end < 0 || !/^\s*\\right/.test(source.slice(end))) continue;
    out += `${source.slice(done, at)}\\kern-0.5em${source.slice(at, end)}\\kern-0.5em`;
    done = end;
  }
  return out + source.slice(done);
}

type Options = Record<string, unknown>;
interface Step {
  tex: string;
  options: Options;
}
interface OptionSet {
  name: string;
  where: string;
  /** The renders the call site makes; a later one runs only if the previous threw. */
  steps: (f: Formula) => Step[];
}

const SETS: OptionSet[] = [
  {
    name: "a",
    where: "widgets.ts:86 (Live render)",
    steps: (f) => [
      { tex: hugArrays(f.tex), options: { displayMode: f.display, throwOnError: true, macros: { "\\arraystretch": "1.2" } } },
    ],
  },
  {
    name: "b",
    where: "mathField.ts:323 (parse gate)",
    steps: (f) => [{ tex: f.tex, options: { displayMode: f.display, throwOnError: true, strict: "ignore" } }],
  },
  {
    name: "c",
    where: "mathTools.ts:77 (palette preview)",
    steps: (f) => [{ tex: f.tex, options: { throwOnError: false } }],
  },
  {
    name: "d",
    where: "mathTools.ts:589 (toolbox preview)",
    steps: (f) => [{ tex: f.tex, options: { displayMode: f.display, throwOnError: true } }],
  },
  {
    name: "e",
    where: "rehype-katex (chat, files)",
    steps: (f) => [
      { tex: f.tex, options: { displayMode: f.display, throwOnError: true } },
      { tex: f.tex, options: { displayMode: f.display, strict: "ignore", throwOnError: false } },
    ],
  },
];

// ── Engines ─────────────────────────────────────────────────────────────────

type Answer = { html: string } | { error: string } | { panic: string };

function renderJs(step: Step): Answer {
  try {
    // A fresh macros object per call: KaTeX writes `\gdef`s into it.
    const options = { ...step.options };
    if (options.macros) options.macros = { ...(options.macros as object) };
    return { html: katex.renderToString(step.tex, options) };
  } catch (e) {
    return { error: e instanceof Error ? e.message : String(e) };
  }
}

async function renderRust(steps: Step[]): Promise<Answer[]> {
  const bin = oracleBin ?? join(CORE, "target/release/oracle");
  const input = steps.map((s, id) => JSON.stringify({ id, tex: s.tex, options: s.options })).join("\n") + "\n";
  const proc = Bun.spawn([bin], { stdin: new Blob([input]), stdout: "pipe", stderr: "ignore" });
  const text = await new Response(proc.stdout).text();
  if ((await proc.exited) !== 0) throw new Error(`oracle exited ${proc.exitCode}`);
  const answers: Answer[] = new Array(steps.length);
  for (const line of text.split("\n")) {
    if (!line) continue;
    const { id, ...rest } = JSON.parse(line);
    answers[id] = rest as Answer;
  }
  return answers;
}

// ── Compare ─────────────────────────────────────────────────────────────────

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

async function main() {
  if (build) {
    const cargo = Bun.spawnSync(["cargo", "build", "--release", "--bin", "oracle"], { cwd: CORE, stdout: "inherit", stderr: "inherit" });
    if (cargo.exitCode !== 0) process.exit(2);
  }
  const formulas = corpus();
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
  const rsList = await renderRust(steps);
  const t2 = performance.now();
  const rs = new Map(steps.map((s, i) => [keyOf(s), rsList[i]]));
  console.log(`renders: ${steps.length} unique; KaTeX JS ${(t1 - t0).toFixed(0)} ms, fork (incl. spawn) ${(t2 - t1).toFixed(0)} ms`);

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
    console.log(`\n${title}\nset  call site                            equal  accepted  html-differs  error-mismatch  error-text-differs  panic`);
    for (const set of sets) {
      const r = rows[set.name + suffix];
      if (!r) continue;
      console.log(
        `${set.name.padEnd(4)} ${set.where.padEnd(36)} ${String(r.equal).padStart(5)}  ${String(r.accepted).padStart(8)}  ${String(r["html-differs"]).padStart(12)}  ${String(r["error-mismatch"]).padStart(14)}  ${String(r["error-text-differs"]).padStart(18)}  ${String(r.panic).padStart(5)}`,
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
  process.exit(diffs.length ? 1 : 0);
}

if (import.meta.main) await main();
