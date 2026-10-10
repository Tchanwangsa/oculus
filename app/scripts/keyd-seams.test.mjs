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
  "keyring::",
];

// The one file allowed to name an otherwise forbidden word: the keychain
// fallback `Secret`, which goes when keyd is always installed.
const ALLOWED = new Map([["src-tauri/src/providers/credentials/keychain.rs", ["keyring::"]]]);

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
  join(app, "src-tauri", "src", "providers", "credentials"),
  join(app, "src-tauri", "src", "auth", "okta"),
  join(app, "src-tauri", "src", "auth", "keyd"),
  join(app, "src-tauri", "src", "bin", "oculus", "commands", "keyd.rs"),
].flatMap(rustFiles);

test("the scope covers keyd's main, core's logic and the app's keyd code", () => {
  const names = scoped.map((f) => relative(app, f));
  for (const f of [
    "keyd/src/main.rs",
    "keyd/core/src/server.rs",
    "keyd/core/src/client.rs",
    "src-tauri/src/auth/keyd/mod.rs",
    "src-tauri/src/auth/okta/mod.rs",
    ...ALLOWED.keys(),
  ]) {
    expect(names).toContain(f);
  }
  expect(names.some((f) => f.includes("/platform/"))).toBe(false);
});

test("no OS call outside platform/", () => {
  const hits = [];
  for (const file of scoped) {
    const allowed = ALLOWED.get(relative(app, file)) ?? [];
    readFileSync(file, "utf8")
      .split("\n")
      .forEach((line, i) => {
        for (const word of FORBIDDEN) {
          if (line.includes(word) && !allowed.includes(word)) {
            hits.push(`${relative(app, file)}:${i + 1}: ${word}`);
          }
        }
      });
  }
  expect(hits).toEqual([]);
});
