// The engines the oracle compares: KaTeX JS (the app's `katex`), the fork as
// the native `oracle` bin, and the fork as the app's wasm build
// (math-core/pkg, from scripts/build-math.mjs).

import katex from "katex";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { Step } from "./sets";

export type Answer = { html: string } | { error: string } | { panic: string };

export function renderJs(step: Step): Answer {
  try {
    // A fresh macros object per call: KaTeX writes `\gdef`s into it.
    const options = { ...step.options };
    if (options.macros) options.macros = { ...(options.macros as object) };
    return { html: katex.renderToString(step.tex, options) };
  } catch (e) {
    return { error: e instanceof Error ? e.message : String(e) };
  }
}

export async function renderNative(steps: Step[], bin: string): Promise<Answer[]> {
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

interface WasmModule {
  initSync(init: { module: BufferSource }): unknown;
  renderToString(tex: string, options?: object): string;
  parseError(tex: string, options?: object): string | undefined;
}

/** What the wasm build does that KaTeX JS's answers can't show. */
export interface WasmChecks {
  /** A throw that is not an `Error` named ParseError with `String(e)` = `ParseError: ${message}`. */
  errorShape: number;
  /** `parseError` disagreeing with `renderToString` (throwOnError steps only). */
  gate: number;
  /** Traps, after each of which the module is instantiated afresh. */
  traps: number;
}

export async function renderWasm(steps: Step[], pkg: string): Promise<{ answers: Answer[]; checks: WasmChecks }> {
  const bytes = readFileSync(join(pkg, "oculus_math_bg.wasm"));
  let instance = 0;
  // A trap leaves the instance's memory mid-call; the query string gives a
  // new copy of the glue, so a fresh instance, rather than the cached one.
  const load = async () => {
    const m = (await import(`${join(pkg, "oculus_math.js")}?instance=${instance++}`)) as WasmModule;
    m.initSync({ module: bytes });
    return m;
  };
  let m = await load();
  const checks: WasmChecks = { errorShape: 0, gate: 0, traps: 0 };
  const answers: Answer[] = [];
  for (const step of steps) {
    let answer: Answer;
    try {
      answer = { html: m.renderToString(step.tex, step.options) };
    } catch (e) {
      if (e instanceof WebAssembly.RuntimeError) {
        checks.traps++;
        answers.push({ panic: `trap: ${e.message}` });
        m = await load();
        continue;
      }
      const shaped = e instanceof Error && e.name === "ParseError" && String(e) === `ParseError: ${e.message}`;
      if (!shaped) checks.errorShape++;
      answer = e instanceof Error && e.name === "ParseError" ? { error: e.message } : { panic: `threw ${String(e)}` };
    }
    if (step.options.throwOnError !== false && !("panic" in answer)) {
      const gate = m.parseError(step.tex, step.options);
      if (gate !== ("error" in answer ? answer.error : undefined)) checks.gate++;
    }
    answers.push(answer);
  }
  return { answers, checks };
}

/** `step` with the fork's source mapping on (the binding's `sourceMap`,
 *  not a KaTeX option). */
export function withSourceMap(step: Step): Step {
  return { tex: step.tex, options: { ...step.options, sourceMap: true } };
}

/** The fork's answers, from the native bin or the wasm build. */
export async function renderFork(
  steps: Step[],
  engine: "native" | "wasm",
  where: { bin: string; pkg: string },
): Promise<{ answers: Answer[]; checks?: WasmChecks }> {
  if (engine === "wasm") return renderWasm(steps, where.pkg);
  return { answers: await renderNative(steps, where.bin) };
}

/** The native bin's typing probe: each formula's every prefix, panics only. */
export async function probePrefixes(
  steps: Step[],
  bin: string,
): Promise<{ prefixes: number; panics: { step: number; len: number; panic: string }[] }> {
  const input = steps.map((s, id) => JSON.stringify({ id, tex: s.tex, options: s.options })).join("\n") + "\n";
  const proc = Bun.spawn([bin, "--prefixes"], { stdin: new Blob([input]), stdout: "pipe", stderr: "ignore" });
  const text = await new Response(proc.stdout).text();
  if ((await proc.exited) !== 0) throw new Error(`oracle exited ${proc.exitCode}`);
  let prefixes = 0;
  const panics: { step: number; len: number; panic: string }[] = [];
  for (const line of text.split("\n")) {
    if (!line) continue;
    const reply = JSON.parse(line) as { id: number; prefixes?: number; panics?: { len: number; panic: string }[]; panic?: string };
    if (reply.panic) throw new Error(`oracle --prefixes: ${reply.panic}`);
    prefixes += reply.prefixes ?? 0;
    for (const p of reply.panics ?? []) panics.push({ step: reply.id, ...p });
  }
  return { prefixes, panics };
}

/** One formula's answer from the bin's `--stops` check. */
export type StopsAnswer =
  | { stops: number; slots: number; between: number; failures: { kind: string; offset: number }[]; parseNs: number }
  | { error: string; parseNs: number }
  | { panic: string };

/** The native bin's edit-field check: each formula's stops and their
 *  invariants (oculus_math_edit::check), with a parse timing. */
export async function checkStops(formulas: { tex: string; display: boolean }[], bin: string): Promise<StopsAnswer[]> {
  const input = formulas.map((f, id) => JSON.stringify({ id, tex: f.tex, display: f.display })).join("\n") + "\n";
  const proc = Bun.spawn([bin, "--stops"], { stdin: new Blob([input]), stdout: "pipe", stderr: "ignore" });
  const text = await new Response(proc.stdout).text();
  if ((await proc.exited) !== 0) throw new Error(`oracle exited ${proc.exitCode}`);
  const answers: StopsAnswer[] = new Array(formulas.length);
  for (const line of text.split("\n")) {
    if (!line) continue;
    const { id, ...rest } = JSON.parse(line);
    answers[id] = rest as StopsAnswer;
  }
  return answers;
}
