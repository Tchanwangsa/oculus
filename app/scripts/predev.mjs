// Dev preflight: dependencies, native binaries, the editor core's wasm, debug CLI, keyd and generated docs.
// OCULUS_CLI_WATCH=1 also starts the CLI watcher; OCULUS_SKIP_PREDEV skips it.
import { execFileSync, spawn } from "node:child_process";
import { existsSync, statSync } from "node:fs";
import { join } from "node:path";
import { buildKeyd, installFromMainCheckout } from "./keyd-build.mjs";
import { app, buildCli, cliPath, stageCli } from "./runtime.mjs";

const cli = cliPath("debug");

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

// 2. The native sidecars. Each script no-ops when its file is already current;
//    the speech helper and whisper-cli are compiled, not fetched, and only on
//    macOS.
for (const script of ["fetch-ffmpeg.mjs", "build-speech.mjs", "build-whisper.mjs"]) {
  execFileSync(process.execPath, [join(app, "scripts", script)], { cwd: app, stdio: "inherit" });
}

// The editor core's wasm for shadow mode (docs/editor-core.md). Without the wasm32
// target or the pinned wasm-bindgen it skips with one line; a failed build
// only leaves shadow mode off, so it never stops the app starting.
try {
  execFileSync(process.execPath, [join(app, "scripts", "build-editor-wasm.mjs"), "--optional"], {
    cwd: app,
    stdio: "inherit",
  });
} catch {
  log("the editor core's wasm did not build — shadow mode stays off");
}

// The credential broker, built and signed every start as its helper app in
// `keyd/target/signed/`, which is the keyd a debug app and CLI offer to
// install. Installing it needs the CLI, so that comes after.
let keyd = null;
try {
  keyd = buildKeyd("dev");
} catch (e) {
  console.error(`[predev] oculus-keyd did not build: ${e.message}`);
  process.exit(1);
}

const started = Date.now();
try {
  buildCli("debug");
} catch (e) {
  console.error(`[predev] the oculus CLI did not build: ${e.message}`);
  process.exit(1);
}

// A binary that builds but cannot answer `--version` is a linker problem
// (sqlite, say) that would otherwise surface as a silent tool failure inside
// an agent's turn, hours later.
let version = "?";
try {
  version = execFileSync(cli, ["--version"], { encoding: "utf8" }).trim();
} catch (e) {
  console.error(`[predev] ${cli} does not run: ${e.message}`);
  process.exit(1);
}
log(`${version} built in ${((Date.now() - started) / 1000).toFixed(1)}s`);

// tauri dev's own cargo run copies the sidecar over this build, so the sidecar
// has to be this build too.
stageCli(cli);

// Installed only from the main checkout and only when its source changed.
// Install runs the CLI built above.
if (keyd) {
  try {
    installFromMainCheckout(keyd.helper);
  } catch (e) {
    console.error(`[predev] oculus-keyd did not install: ${e.message}`);
    process.exit(1);
  }
}

// Regenerate the reference from this build, writing only changed help.
try {
  execFileSync(process.execPath, [join(app, "scripts", "gen-cli-docs.mjs")], {
    cwd: app,
    stdio: "inherit",
    env: { ...process.env, OCULUS_BIN: cli },
  });
} catch {
  log("could not regenerate docs/cli-reference.md — carrying on");
}

// Refresh the library's agent docs. A fresh machine has no data directory,
// so failure here must not prevent the app from starting.
try {
  execFileSync(cli, ["docs"], { stdio: "inherit" });
} catch {
  log("could not refresh the library's agent docs — carrying on");
}

// Keep the CLI current because tauri dev does not re-run this preflight.
if (watch) {
  const child = spawn(process.execPath, [join(app, "scripts", "watch-cli.mjs")], {
    cwd: app,
    detached: true,
    stdio: ["ignore", "inherit", "inherit"],
  });
  child.unref();
}
