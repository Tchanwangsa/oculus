// Build, sign and — from the main checkout, when its source changed — install
// oculus-keyd. macOS only. The build is reproducible: the same source gives the
// same bytes from any checkout, so the signed cdhash the keychain trusts only
// changes when keyd's source does (docs/development.md).
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, realpathSync, renameSync, rmSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { app, buildCli, cliPath } from "./runtime.mjs";

const log = (msg) => console.log(`[keyd] ${msg}`);

if (process.platform !== "darwin") {
  log("keyd runs under launchd, so it is macOS-only — nothing built");
  process.exit(0);
}

const crate = realpathSync(join(app, "keyd"));
const home = homedir();
const cargoHome = process.env.CARGO_HOME ?? join(home, ".cargo");
const rustupHome = process.env.RUSTUP_HOME ?? join(home, ".rustup");

// Path-dependent half of the reproducible flags; keyd/.cargo/config.toml has
// the rest. Cargo joins a --config array onto the config file's, and reads
// that file only when run from the crate, hence the cwd.
const remaps = [[crate, "/keyd"], [cargoHome, "/cargo"], [rustupHome, "/rustup"]]
  .filter(([from]) => existsSync(from))
  .map(([from, to]) => `--remap-path-prefix=${realpathSync(from)}=${to}`);
const rustflags = `build.rustflags=[${remaps.map((f) => JSON.stringify(f)).join(", ")}]`;

const built = join(crate, "target", "release", "oculus-keyd");
rmSync(built, { force: true });
execFileSync("cargo", ["build", "--release", "--locked", "--features", "dev", "--config", rustflags], {
  cwd: crate,
  stdio: "inherit",
});
if (!existsSync(built)) throw new Error(`cargo reported success but ${built} is not there`);

// Sign a fresh copy, then rename it into place: re-signing a file that has
// already run leaves the kernel's cached signature stale. `-i` keeps the
// cdhash independent of the file name.
const signedDir = join(crate, "target", "signed");
const signed = join(signedDir, "oculus-keyd");
const partial = `${signed}.${process.pid}.part`;
mkdirSync(signedDir, { recursive: true });
try {
  copyFileSync(built, partial);
  execFileSync("codesign", ["-s", "-", "-f", "-o", "runtime", "-i", "com.tchan.oculus.keyd", partial], { stdio: "inherit" });
  renameSync(partial, signed);
} finally {
  rmSync(partial, { force: true });
}
const hash = execFileSync(signed, ["source-hash"], { encoding: "utf8" }).trim();
log(`built and signed ${signed} (source ${hash.slice(0, 12)})`);

// Only the main checkout installs: a worktree's build must never take over the
// LaunchAgent the running app depends on.
const git = (...args) =>
  execFileSync("git", ["rev-parse", "--path-format=absolute", ...args], {
    cwd: app,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  }).trim();
let main = false;
try {
  main = realpathSync(git("--git-dir")) === realpathSync(git("--git-common-dir"));
} catch {
  // No git, or not a checkout: nothing to install from.
}
if (!main) {
  log(`not the main checkout — not installed (by hand: oculus keyd install --from ${signed})`);
  process.exit(0);
}

const dataDir = join(home, "Library", "Application Support", "com.tchan.oculus");
let installed = "";
try {
  installed = readFileSync(join(dataDir, "bin", "oculus-keyd.stamp"), "utf8").trim();
} catch {
  // Never installed.
}
if (installed === hash) {
  log("the installed keyd is this source — nothing to install");
  process.exit(0);
}

const cli = process.env.OCULUS_BIN ?? cliPath("debug");
if (!existsSync(cli)) buildCli("debug");
execFileSync(cli, ["keyd", "install", "--from", signed], { stdio: "inherit" });
