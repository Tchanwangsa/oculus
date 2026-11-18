// Gives a fresh git worktree the main checkout's gitignored build inputs and a
// warm Rust build, so its first cargo run recompiles only the crates whose
// path differs (vendored wry and what depends on it, the app) instead of all
// of them. docs/development.md, "A new worktree starts from the main build".
import { execFileSync } from "node:child_process";
import { existsSync, lstatSync, mkdirSync, readdirSync, rmSync, symlinkSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";

// The checkout is found from the working directory, so the main checkout's
// copy can warm a worktree whose branch predates this script.
const git = (...args) => execFileSync("git", args, { encoding: "utf8" }).trim();
const app = join(git("rev-parse", "--show-toplevel"), "app");
const gitDir = resolve(git("rev-parse", "--git-dir"));
const commonDir = resolve(git("rev-parse", "--git-common-dir"));
if (gitDir === commonDir) {
  console.log("warm-worktree: this is the main checkout, nothing to do");
  process.exit(0);
}
const mainApp = join(dirname(commonDir), "app");

// Directory patterns in .gitignore don't match symlinks; the common
// info/exclude lists these two so they never show as untracked.
for (const path of ["node_modules", "src-tauri/binaries"]) {
  const here = join(app, path);
  const there = join(mainApp, path);
  if (exists(here) || !existsSync(there)) continue;
  symlinkSync(there, here);
  console.log(`warm-worktree: linked ${path}`);
}

const from = join(mainApp, "src-tauri/target/debug");
const to = join(app, "src-tauri/target/debug");
if (process.platform !== "darwin") {
  console.log("warm-worktree: build cloning needs APFS clones (macOS); skipped");
} else if (existsSync(to)) {
  console.log("warm-worktree: target/debug already exists, left alone");
} else if (!existsSync(join(from, "deps"))) {
  console.log(`warm-worktree: ${relative(app, from)} has no build to clone`);
} else {
  const started = Date.now();
  try {
    clone(from, to);
  } catch (error) {
    // A half-cloned build would look fresh to cargo.
    rmSync(to, { recursive: true, force: true });
    throw error;
  }
  console.log(`warm-worktree: cloned the main build in ${Date.now() - started} ms`);
}

function clone(from, to) {
  mkdirSync(join(to, "deps"), { recursive: true });
  execFileSync("cp", ["-cR", join(from, ".fingerprint"), join(from, "build"), to]);
  // deps/ holds hundreds of thousands of the app crate's leftover `.o` debug
  // objects, which cost minutes to clone and are rebuilt anyway.
  // Directories there are rustc's scratch space for builds in flight.
  const files = readdirSync(join(from, "deps"), { withFileTypes: true })
    .filter((entry) => entry.isFile() && !entry.name.endsWith(".o"))
    .map((entry) => entry.name);
  for (let i = 0; i < files.length; i += 500) {
    const batch = files.slice(i, i + 500).map((name) => join(from, "deps", name));
    execFileSync("cp", ["-c", ...batch, join(to, "deps")]);
  }
}

function exists(path) {
  try {
    lstatSync(path);
    return true;
  } catch {
    return false;
  }
}
