// The oracle's inputs: every maths span in the user's notes (read at run
// time, never copied), KaTeX's pinned spec fixtures, optional spec inputs from
// a KaTeX checkout, and our synthetic fixtures.json.

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

const HERE = import.meta.dir;
const CORE = resolve(HERE, "..");

export type Source = "fixture" | "notes" | "spec";
export interface Formula {
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

function specInputs(katexDir: string | undefined): Formula[] {
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

/** Unique (display, tex) pairs; the first source to name one keeps it. */
export function corpus(notesDir: string, katexDir?: string): Formula[] {
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
  for (const f of specInputs(katexDir)) add(f);
  return [...seen.values()];
}
