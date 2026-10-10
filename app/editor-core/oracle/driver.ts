// Shared driver for the oracle scripts: argument parsing, a seeded rng per case
// (so one case replays from its seed and index), the cargo build of the Rust
// `oracle` binary, batched runs over JSON lines, a structural diff, the corpus
// loader, and a mismatch printer that never prints a whole document.
//
// An area script builds `{ request, expected }` pairs, hands them to a
// `Checker`, and calls `finish()`. Requests carry `"op"`; see src/bin/oracle/.

import { readdir, readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join, resolve } from "node:path";

export const crate = resolve(import.meta.dir, "..");

export interface Args {
  cases: number;
  seed: number;
  /** Replay only this case index. */
  only: number | null;
}

/** `bun <script> [cases] [seed] [only]`. Each must be a non-negative integer,
 * and `only` must select one of the `cases`. */
export function parseArgs(defaultCases: number): Args {
  const [cases, seed, only, ...extra] = process.argv.slice(2);
  const int = (name: string, value: string | undefined) => {
    if (value === undefined) return null;
    if (!/^\d+$/.test(value) || !Number.isSafeInteger(Number(value))) {
      usage(`${name} must be a non-negative integer, got ${JSON.stringify(value)}`);
    }
    return Number(value);
  };
  if (extra.length) usage(`unexpected arguments ${JSON.stringify(extra)}`);
  const args = {
    cases: int("cases", cases) ?? defaultCases,
    seed: int("seed", seed) ?? Math.floor(Math.random() * 2 ** 31),
    only: int("only", only),
  };
  if (args.only !== null && args.only >= args.cases) {
    usage(`only ${args.only} selects nothing: cases are 0..${args.cases - 1}`);
  }
  return args;
}

function usage(message: string): never {
  console.error(`${message}\nusage: bun ${process.argv[1]} [cases] [seed] [only]`);
  process.exit(2);
}

export class Rng {
  private s: number;

  constructor(seed: number) {
    this.s = seed | 0;
  }

  /** mulberry32: uniform in [0, 1). */
  next(): number {
    this.s = (this.s + 0x6d2b79f5) | 0;
    let t = Math.imul(this.s ^ (this.s >>> 15), 1 | this.s);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  }

  /** Integer in [0, n). */
  int(n: number): number {
    return Math.floor(this.next() * n);
  }

  /** Integer in [lo, hi], log-uniform: small values as likely as large ones. */
  logInt(lo: number, hi: number): number {
    return Math.floor(Math.exp(Math.log(lo) + this.next() * (Math.log(hi + 1) - Math.log(lo))));
  }

  chance(p: number): boolean {
    return this.next() < p;
  }

  pick<T>(items: readonly T[]): T {
    return items[this.int(items.length)];
  }
}

/** The rng seed of case `index` in a run seeded with `seed`. */
export function caseRng(seed: number, index: number): Rng {
  let h = Math.imul(seed ^ 0x9e3779b9, 0x85ebca6b) ^ Math.imul(index + 1, 0xc2b2ae35);
  h = Math.imul(h ^ (h >>> 16), 0x7feb352d);
  return new Rng(h ^ (h >>> 15));
}

/** Builds the release oracle binary and returns its path. */
export function buildOracle(): string {
  const build = Bun.spawnSync(
    ["cargo", "build", "--release", "--quiet", "--features", "oracle", "--bin", "oracle"],
    { cwd: crate, stdout: "inherit", stderr: "inherit" },
  );
  if (build.exitCode !== 0) process.exit(build.exitCode ?? 1);
  return join(crate, "target/release/oracle");
}

/** Every note under the app's data directory, read at run time, never stored. */
export async function corpus(): Promise<{ name: string; source: string }[]> {
  const root = join(homedir(), "Library/Application Support/com.tchan.oculus/courses");
  const out: { name: string; source: string }[] = [];
  let courses: string[];
  try {
    courses = await readdir(root);
  } catch {
    return out;
  }
  for (const course of courses.sort()) {
    const dir = join(root, course, "documents");
    let files: string[];
    try {
      files = await readdir(dir);
    } catch {
      continue;
    }
    for (const file of files.filter((f) => f.endsWith(".md")).sort()) {
      out.push({ name: `${course}/${file}`, source: await readFile(join(dir, file), "utf8") });
    }
  }
  return out;
}

export interface Diff {
  path: string;
  expected: unknown;
  actual: unknown;
}

/** The first place `actual` differs from `expected`, or null. */
export function firstDiff(expected: unknown, actual: unknown, path = ""): Diff | null {
  if (Object.is(expected, actual)) return null;
  if (Array.isArray(expected) && Array.isArray(actual)) {
    for (let i = 0; i < Math.max(expected.length, actual.length); i++) {
      if (i >= expected.length || i >= actual.length) {
        return { path: `${path}.length`, expected: expected.length, actual: actual.length };
      }
      const d = firstDiff(expected[i], actual[i], `${path}[${i}]`);
      if (d) return d;
    }
    return null;
  }
  if (isObject(expected) && isObject(actual)) {
    const keys = new Set([...Object.keys(expected), ...Object.keys(actual)]);
    for (const key of [...keys].sort()) {
      const d = firstDiff(expected[key], actual[key], `${path}.${key}`);
      if (d) return d;
    }
    return null;
  }
  return { path: path || "(root)", expected, actual };
}

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

/** A short, JSON-escaped view of a value; for two strings, ±40 units around
 * their first difference. */
function excerpts(expected: unknown, actual: unknown): [string, string] {
  if (typeof expected === "string" && typeof actual === "string") {
    let i = 0;
    while (i < expected.length && i < actual.length && expected[i] === actual[i]) i++;
    const cut = (s: string) =>
      `${i > 40 ? "…" : ""}${JSON.stringify(s.slice(Math.max(0, i - 40), i + 40))}${i + 40 < s.length ? "…" : ""}` +
      ` (len ${s.length}, differs at ${i})`;
    return [cut(expected), cut(actual)];
  }
  const short = (v: unknown) => {
    const s = JSON.stringify(v) ?? String(v);
    return s.length > 160 ? `${s.slice(0, 160)}… (${s.length} chars)` : s;
  };
  return [short(expected), short(actual)];
}

/** Sends JSON lines to the oracle and returns its answer lines. */
function runLines(binary: string, requests: string[], context: string): string[] {
  const payload = requests.join("\n") + "\n";
  const run = Bun.spawnSync([binary], { stdin: Buffer.from(payload), stdout: "pipe", stderr: "inherit" });
  if (run.exitCode !== 0) {
    console.error(`${context}: oracle exited ${run.exitCode}`);
    process.exit(1);
  }
  return new TextDecoder().decode(run.stdout).trimEnd().split("\n");
}

/** Asks the oracle questions that have no CodeMirror counterpart (the Rust
 * tree's shape, say) and returns the parsed answers in order. */
export function query(binary: string, requests: object[]): any[] {
  if (requests.length === 0) return [];
  const answers = runLines(binary, requests.map((r) => JSON.stringify(r)), "query");
  return answers.map((line) => {
    const answer = JSON.parse(line);
    if (answer && typeof answer === "object" && "error" in answer) {
      console.error(`query refused: ${answer.error}`);
      process.exit(1);
    }
    return answer;
  });
}

interface Pending {
  label: string;
  replay: string;
  request: string;
  /** JSON, parsed back only when its answer arrives: a parsed snapshot holds
   * several times its text in memory. */
  expected: string;
}

/** Characters of requests plus expected answers per batch. The oracle's
 * answers are about the size of the expected ones, so a batch holds roughly
 * twice this in text at its peak. */
const BATCH_CHARS = 16e6;

/** Runs requests through the oracle in batches and diffs every answer. */
export class Checker {
  private batch: Pending[] = [];
  private batchChars = 0;
  private checked = 0;
  private failures = 0;

  constructor(
    private readonly name: string,
    private readonly binary: string,
    private readonly seed: number,
    private readonly maxShown = 5,
  ) {}

  /** `replay` is the command line that regenerates this case alone. */
  add(label: string, replay: string, request: object, expected: unknown) {
    const line = JSON.stringify(request);
    const want = JSON.stringify(expected) ?? "null";
    this.batch.push({ label, replay, request: line, expected: want });
    this.batchChars += line.length + want.length;
    if (this.batchChars > BATCH_CHARS || this.batch.length >= 5000) this.flush();
  }

  flush() {
    if (this.batch.length === 0) return;
    const batch = this.batch;
    this.batch = [];
    this.batchChars = 0;
    const answers = runLines(this.binary, batch.map((p) => p.request), `${this.name} (seed ${this.seed})`);
    batch.forEach((p, i) => {
      const expected = JSON.parse(p.expected);
      const actual = answers[i] === undefined ? null : JSON.parse(answers[i]);
      const refused = isObject(actual) && "error" in actual && !(isObject(expected) && "error" in expected);
      const diff = refused ? { path: ".error", expected: undefined, actual: actual.error } : firstDiff(expected, actual);
      this.checked++;
      if (!diff) return;
      this.failures++;
      if (this.failures > this.maxShown) return;
      const [want, got] = excerpts(diff.expected, diff.actual);
      console.error(`MISMATCH ${p.label} at ${diff.path}`);
      console.error(`  expected: ${want}`);
      console.error(`  actual:   ${got}`);
      console.error(`  replay:   ${p.replay}`);
    });
  }

  /** Flushes, prints the summary line, and exits non-zero on any mismatch or
   * when no random case was checked (an empty run proves nothing). */
  finish(summary: string, randomCases: number): never {
    this.flush();
    console.log(`${this.name}: ${summary}, ${this.checked} requests, seed ${this.seed}, ${this.failures} mismatches`);
    if (randomCases === 0) {
      console.error(`${this.name}: no random case was checked`);
      process.exit(1);
    }
    process.exit(this.failures === 0 ? 0 : 1);
  }
}
