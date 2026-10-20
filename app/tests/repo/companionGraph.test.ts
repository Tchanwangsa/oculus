import { expect, test } from "bun:test";
import { existsSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";

// The browser companion (`src/companion/`, docs/frontend.md) runs on a paired
// device with no Tauri: nothing it imports, statically or lazily, may reach the
// Tauri API, the SQL plugin or the database layer. The desktop entry, in turn,
// never pulls the companion in.

const APP = join(import.meta.dir, "../..");
const SRC = join(APP, "src");
/** `vite.companion.config.ts` swaps these modules for browser stand-ins. */
const COMPANION_ALIASES: Record<string, string> = { "@/lib/menu/editRouting": "src/companion/editRouting.ts" };

const IMPORT = /(?:^|[;\n}])\s*(?:import|export)\s+(type\s+)?(?:[^'"`;]*?\s+from\s+)?["']([^"']+)["']|import\(\s*["']([^"']+)["']\s*\)/g;
const FORBIDDEN = (spec: string) => spec.startsWith("@tauri-apps/") || spec === "@/lib/db" || spec.startsWith("@/lib/db/") || spec.includes("plugin-sql");

function resolve(from: string, spec: string, aliases: Record<string, string>): string | null {
  let base: string;
  if (aliases[spec]) base = join(APP, aliases[spec]);
  else if (spec.startsWith("@/")) base = join(SRC, spec.slice(2));
  else if (spec.startsWith(".")) base = join(dirname(from), spec);
  else return null;
  for (const candidate of [base, `${base}.ts`, `${base}.tsx`, join(base, "index.ts"), join(base, "index.tsx")]) {
    if (existsSync(candidate) && statSync(candidate).isFile()) return candidate;
  }
  return null;
}

/** Every local module reachable from `entry`, and each import that names a forbidden module, with the path to it. */
function walk(entry: string, aliases: Record<string, string> = {}) {
  const parent = new Map<string, string | null>([[join(APP, entry), null]]);
  const queue = [join(APP, entry)];
  const violations: string[] = [];
  const chain = (file: string) => {
    const path: string[] = [];
    for (let at: string | null | undefined = file; at; at = parent.get(at)) path.unshift(relative(APP, at));
    return path.join(" > ");
  };
  while (queue.length) {
    const file = queue.shift()!;
    if (!/\.(ts|tsx)$/.test(file)) continue;
    for (const match of readFileSync(file, "utf8").matchAll(IMPORT)) {
      if (match[1]) continue; // `import type` is erased
      const spec = match[2] ?? match[3];
      if (FORBIDDEN(spec)) violations.push(`${chain(file)} imports ${spec}`);
      const next = resolve(file, spec, aliases);
      if (next && !parent.has(next)) { parent.set(next, file); queue.push(next); }
    }
  }
  return { modules: [...parent.keys()].map((file) => relative(APP, file)), violations };
}

test("the companion's module graph never reaches Tauri or the database", () => {
  const { modules, violations } = walk("src/companion/main.tsx", COMPANION_ALIASES);
  expect(modules).toContain("src/components/canvas/CanvasSurface.tsx");
  expect(modules).toContain("src/lib/companion/shareClient.ts");
  expect(violations).toEqual([]);
});

test("the companion build applies the aliases this test assumes", () => {
  const config = readFileSync(join(APP, "vite.companion.config.ts"), "utf8");
  for (const [from, to] of Object.entries(COMPANION_ALIASES)) {
    expect(config).toContain(from.replaceAll("/", "\\/"));
    expect(config).toContain(`./${to}`);
  }
});

test("the desktop entry does not bundle the companion page or its transport", () => {
  const { modules } = walk("src/main.tsx");
  expect(modules.filter((file) => file.startsWith("src/companion/") || file.startsWith("src/lib/companion/shareClient"))).toEqual([]);
});
