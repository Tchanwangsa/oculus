#!/usr/bin/env node
// Everything `tauri dev` needs on disk before the app comes up.
//
// Named for bun's lifecycle, not by coincidence: `bun run dev` runs `predev`
// first, and `bun run dev` is what `beforeDevCommand` calls — so the preflight
// is not a step anyone can forget to add to the chain. `OCULUS_CLI_WATCH=1`
// (set in beforeDevCommand) also leaves the CLI watcher running behind it;
// `OCULUS_SKIP_PREDEV=1` skips the whole thing for a vite-only session.
//
// **The step that earns this script is the `oculus` CLI.** It is a second
// binary in the same crate, and nothing in the dev path ever built it:
// `tauri dev` issues a bare `cargo run`, which builds `app` and no other bin
// target, and `bun run cli` builds the *release* one by hand. So
// `target/debug/oculus` sat at whatever the CLI looked like the last time
// somebody ran `cargo test` — measured, ten days and several subcommands
// stale — while being the sibling of the running app and therefore the first
// `oculus` on a coding agent's PATH (`child_env` in
// src-tauri/src/harness/discover.rs). The agent called `oculus project create`
// and got `unrecognized subcommand`, from a binary that looked entirely
// legitimate. Building it here costs ~25s after a Rust change and ~3s when
// nothing moved, because the library it links is the one the app build is
// about to compile anyway.
//
// Debug, not release, is deliberate: the dev app runs out of `target/debug`,
// so this is the profile whose artifacts are already warm. A release CLI in
// front of every dev start would be a minute of optimising a binary the dev
// session does not use.

import { execFileSync, spawn } from "node:child_process";
import { existsSync, rmSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const app = dirname(dirname(fileURLToPath(import.meta.url)));
const manifest = join(app, "src-tauri", "Cargo.toml");
const exe = process.platform === "win32" ? ".exe" : "";
const cli = join(app, "src-tauri", "target", "debug", `oculus${exe}`);

const watch = process.argv.includes("--watch") || process.env.OCULUS_CLI_WATCH === "1";
const log = (msg) => console.log(`[predev] ${msg}`);

if (process.env.OCULUS_SKIP_PREDEV) {
  log("OCULUS_SKIP_PREDEV is set — nothing built");
  process.exit(0);
}

function mtime(p) {
  try {
    return statSync(p).mtimeMs;
  } catch {
    return 0;
  }
}

// 1. Dependencies. `--frozen-lockfile` so this can only ever install what
//    app/bun.lock already says — a dev start is not the place to resolve new
//    versions.
const modules = join(app, "node_modules");
if (!existsSync(modules) || mtime(modules) < mtime(join(app, "bun.lock"))) {
  log("bun install");
  execFileSync("bun", ["install", "--frozen-lockfile"], { cwd: app, stdio: "inherit" });
}

// 2. The fetched natives. Both scripts no-op when the file is already there.
for (const script of ["fetch-ffmpeg.mjs", "fetch-pdfium.mjs"]) {
  execFileSync(process.execPath, [join(app, "scripts", script)], { cwd: app, stdio: "inherit" });
}

// 3. The CLI. The `rm` is not superstition: cargo reports "Finished" and
//    leaves the previous binary in place when it decides the uplift from
//    target/debug/deps is unnecessary, which is the exact failure this script
//    exists to prevent.
const started = Date.now();
rmSync(cli, { force: true });
try {
  execFileSync("cargo", ["build", "--manifest-path", manifest, "--bin", "oculus"], {
    stdio: "inherit",
  });
} catch {
  console.error("[predev] the oculus CLI did not build — fix the error above");
  process.exit(1);
}
if (!existsSync(cli)) {
  console.error(`[predev] cargo reported success but ${cli} is not there`);
  process.exit(1);
}

// A binary that builds but cannot answer `--version` is a linker problem
// (pdfium, sqlite) that would otherwise surface as a silent tool failure
// inside an agent's turn, hours later.
let version = "?";
try {
  version = execFileSync(cli, ["--version"], { encoding: "utf8" }).trim();
} catch (e) {
  console.error(`[predev] ${cli} does not run: ${e.message}`);
  process.exit(1);
}
log(`${version} built in ${((Date.now() - started) / 1000).toFixed(1)}s`);

// 4. The generated reference, from the binary that was just built. It rewrites
//    docs/cli-reference.md only when the help actually changed, so a clean tree
//    stays clean — and when it does change, the doc and the code it describes
//    are dirty in the same commit, which is the rule in the root CLAUDE.md.
try {
  execFileSync(process.execPath, [join(app, "scripts", "gen-cli-docs.mjs")], {
    cwd: app,
    stdio: "inherit",
    env: { ...process.env, OCULUS_BIN: cli },
  });
} catch {
  log("could not regenerate docs/cli-reference.md — carrying on");
}

// 5. The session watcher. `tauri dev` rebuilds and relaunches the app on a
//    Rust change but never re-runs this hook, so without it the CLI is only as
//    fresh as the moment the session started.
if (watch) {
  const child = spawn(process.execPath, [join(app, "scripts", "watch-cli.mjs")], {
    cwd: app,
    detached: true,
    stdio: ["ignore", "inherit", "inherit"],
  });
  child.unref();
}
