import { expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join, relative, sep } from "node:path";

// Oculus works at any university (docs/auth.md#oculus-works-at-any-university):
// no university's host, and no one region of a regional service, in app code.
// Tests, fixtures and comments may name UniMelb; app code reads the
// institution profile instead.

const APP = join(import.meta.dir, "../..");
const ROOTS = ["src", "src-tauri/src", "keyd/src", "keyd/core/src"];

/** The only files whose code may name a university or one region. */
const ALLOWED = new Set([
  // The profile an install from before `institution.json` was built against.
  "src-tauri/src/sources/institution/mod.rs",
  // keyd's site for a vault that was never told one, for the same installs.
  "keyd/core/src/site.rs",
  // Echo360's regions, all of them.
  "src-tauri/src/sources/echo360/embed/region.rs",
]);

const FORBIDDEN: [RegExp, string][] = [
  [/\b[a-z0-9-]+\.(edu(\.[a-z]{2})?|ac\.[a-z]{2})\b/i, "a university's domain"],
  [/echo360\.(org|net|ca)/i, "one Echo360 region"],
  [/edstem\.org\/(au|us|ca|eu)\b|\.(au|us|ca|eu)\.edstem\.org/i, "one Ed region"],
];

/** Placeholders the UI shows, which name no university. */
const PLACEHOLDER = /\b(youruni|example)\.edu\b/gi;

function* sources(dir: string): Generator<string> {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === "tests" || entry.name === "fixtures") continue;
      yield* sources(path);
    } else if (/\.(rs|ts|tsx)$/.test(entry.name) && !/(^tests|_tests|\.test)\.(rs|ts|tsx)$/.test(entry.name)) {
      yield path;
    }
  }
}

/** Code lines: comments dropped, and a Rust file's inline tests (which sit
 *  after its `#[cfg(test)]`) left out. */
function codeLines(path: string): [number, string][] {
  const lines = readFileSync(path, "utf8").split("\n");
  const end = path.endsWith(".rs") ? lines.findIndex((l) => l.trim() === "#[cfg(test)]") : -1;
  return lines
    .slice(0, end === -1 ? lines.length : end)
    .map((line, i): [number, string] => [i + 1, line])
    .filter(([, line]) => !/^\s*(\/\/|\/?\*)/.test(line));
}

test("app code names no university and no single region of a service", () => {
  const hits: string[] = [];
  for (const root of ROOTS) {
    for (const path of sources(join(APP, root))) {
      // `ALLOWED` is written with `/`; Windows paths come back with `\`.
      const rel = relative(APP, path).split(sep).join("/");
      if (ALLOWED.has(rel)) continue;
      for (const [n, line] of codeLines(path)) {
        const stripped = line.replace(PLACEHOLDER, "");
        for (const [pattern, what] of FORBIDDEN) {
          if (pattern.test(stripped)) hits.push(`${rel}:${n} names ${what}: ${line.trim()}`);
        }
      }
    }
  }
  expect(hits).toEqual([]);
});
