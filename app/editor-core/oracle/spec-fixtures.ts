// Regenerates the spec-test fixtures under tests/fixtures/ (vendored; see
// NOTICE for their licences). Rarely needed. Each source must already be
// on the machine; the script stops with a message if one is missing:
// - commonmark-spec.json: the CommonMark 0.31.2 examples, from the Go module
//   cache's goldmark v1.7.13 (`go mod download github.com/yuin/goldmark@v1.7.13`),
//   file `_test/spec.json`;
// - gfm-examples.json: the GFM table, strikethrough and task-list examples
//   in pulldown-cmark 0.13.4's generated tests, from the cargo registry
//   (there after `cargo fetch` in any crate depending on it),
//   `tests/suite/gfm_{table,strikethrough,tasklist}.rs`;
// - entities.json: HTML named character references from the app's
//   `character-entities` package.
// Override the locations with GOLDMARK_DIR and PULLDOWN_DIR.
//
//   bun editor-core/oracle/spec-fixtures.ts      (from app/)

import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

import { characterEntities } from "character-entities";

import { crate } from "./driver";

const out = join(crate, "tests/fixtures");
const goldmark = process.env.GOLDMARK_DIR ?? join(homedir(), "go/pkg/mod/github.com/yuin/goldmark@v1.7.13");
const pulldownDir =
  process.env.PULLDOWN_DIR ?? join(homedir(), ".cargo/registry/src/index.crates.io-1949cf8c6b5b557f/pulldown-cmark-0.13.4");
const spec = join(goldmark, "_test/spec.json");
const pulldown = join(pulldownDir, "tests/suite");
for (const [what, path] of [["goldmark v1.7.13's spec.json", spec], ["pulldown-cmark 0.13.4's tests", pulldown]]) {
  if (!existsSync(path)) {
    console.error(`spec-fixtures: ${what} not found at ${path}; see this script's header`);
    process.exit(1);
  }
}

await Bun.write(join(out, "commonmark-spec.json"), readFileSync(spec, "utf8"));

const gfm: { name: string; markdown: string; html: string }[] = [];
for (const section of ["gfm_table", "gfm_strikethrough", "gfm_tasklist"]) {
  const src = readFileSync(join(pulldown, `${section}.rs`), "utf8");
  const re = /fn (\w+)\(\) \{\s*let original = r##"([\s\S]*?)"##;\s*let expected = r##"([\s\S]*?)"##;/g;
  for (let m; (m = re.exec(src)); ) gfm.push({ name: m[1], markdown: m[2], html: m[3] });
}
await Bun.write(join(out, "gfm-examples.json"), `${JSON.stringify(gfm, null, 1)}\n`);

const names = Object.keys(characterEntities).sort();
await Bun.write(
  join(out, "entities.json"),
  `{\n${names.map((n) => `${JSON.stringify(n)}:${JSON.stringify(characterEntities[n])}`).join(",\n")}\n}\n`,
);
console.log(`fixtures: spec copied, ${gfm.length} GFM examples, ${names.length} entities`);
