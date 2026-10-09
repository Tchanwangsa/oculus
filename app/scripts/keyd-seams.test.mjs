// No OS call outside keyd's adapters (`app/keyd/core/src/platform/`): keyd's
// main, core's logic and the app's keyd code name no Unix, libc, framework
// or launchd API, so another OS needs only a new adapter
// (docs/development.md). Comments count too: a doc that names launchd's
// files belongs in the adapter or in docs/.
import { expect, test } from "bun:test";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { app } from "./runtime.mjs";

const FORBIDDEN = [
  "std::os::unix",
  "std::os::fd",
  "libc::",
  "core_foundation",
  "security_framework",
  'extern "C"',
  "#[link",
  "launchctl",
  "LaunchAgents",
  "Library/Application Support",
];

const platform = join(app, "keyd", "core", "src", "platform");

function rustFiles(path) {
  if (!existsSync(path)) return [];
  if (!statSync(path).isDirectory()) return path.endsWith(".rs") ? [path] : [];
  if (path === platform) return [];
  return readdirSync(path).flatMap((name) => rustFiles(join(path, name)));
}

const scoped = [
  join(app, "keyd", "src"),
  join(app, "keyd", "core", "src"),
  join(app, "src-tauri", "src", "credentials"),
  join(app, "src-tauri", "src", "credentials.rs"),
  join(app, "src-tauri", "src", "keyd.rs"),
  join(app, "src-tauri", "src", "bin", "oculus", "keyd.rs"),
].flatMap(rustFiles);

test("the scope covers keyd's main, core's logic and the app's keyd code", () => {
  const names = scoped.map((f) => relative(app, f));
  for (const f of ["keyd/src/main.rs", "keyd/core/src/server.rs", "keyd/core/src/client.rs", "src-tauri/src/keyd.rs"]) {
    expect(names).toContain(f);
  }
  expect(names.some((f) => f.includes("/platform/"))).toBe(false);
});

test("no OS call outside platform/", () => {
  const hits = [];
  for (const file of scoped) {
    readFileSync(file, "utf8")
      .split("\n")
      .forEach((line, i) => {
        for (const word of FORBIDDEN) {
          if (line.includes(word)) hits.push(`${relative(app, file)}:${i + 1}: ${word}`);
        }
      });
  }
  expect(hits).toEqual([]);
});
