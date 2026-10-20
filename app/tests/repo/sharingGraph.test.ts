import { expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";

// `lib/sharing/` is the desktop's side of device sharing and calls Tauri; the
// browser companion never imports it. `lib/qr/` stays free of any app import,
// so either side can draw a code.

const APP = join(import.meta.dir, "../..");

function* files(dir: string): Generator<string> {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* files(path);
    else if (/\.(ts|tsx)$/.test(entry.name)) yield path;
  }
}

const imports = (path: string) => [...readFileSync(path, "utf8").matchAll(/(?:from\s+|import\(\s*)["']([^"']+)["']/g)].map((m) => m[1]);

test("the companion never imports the desktop's sharing module", () => {
  const offenders: string[] = [];
  for (const root of ["src/companion", "src/lib/companion"]) {
    for (const path of files(join(APP, root))) {
      for (const spec of imports(path)) if (/(^@\/lib\/sharing|\/sharing\/)/.test(spec)) offenders.push(`${relative(APP, path)} imports ${spec}`);
    }
  }
  expect(offenders).toEqual([]);
});

test("the QR encoder imports nothing outside itself", () => {
  const outside: string[] = [];
  for (const path of files(join(APP, "src/lib/qr"))) {
    for (const spec of imports(path)) if (!spec.startsWith("./")) outside.push(`${relative(APP, path)} imports ${spec}`);
  }
  expect(outside).toEqual([]);
});
