// Build the maths engine (math-core/wasm: the katex fork and the edit model) to
// WebAssembly in math-core/pkg: cargo → wasm-bindgen --target web → wasm-opt
// -Oz. Needs the wasm32-unknown-unknown target and the wasm-bindgen CLI at
// Cargo.lock's version; wasm-opt is the binaryen devDependency. Skipped while
// the .wasm is newer than every input.
import { execFileSync } from "node:child_process";
import {
  copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, renameSync, statSync,
} from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { app } from "./runtime.mjs";

const core = join(app, "math-core");
const pkg = join(core, "pkg");
const wasm = join(pkg, "oculus_math_bg.wasm");
const glue = ["oculus_math.js", "oculus_math.d.ts", "oculus_math_bg.wasm.d.ts"];
const target = "wasm32-unknown-unknown";
const built = join(core, "target", target, "wasm", "oculus_math.wasm");
const staging = join(core, "target", "wasm-bindgen");

const log = (msg) => console.log(`[math] ${msg}`);
const fail = (msg) => {
  console.error(`[math] ${msg}`);
  process.exit(1);
};

function mtime(p) {
  try {
    return statSync(p).mtimeMs;
  } catch {
    return 0;
  }
}

/** Newest mtime under `dir`, skipping the top-level names in `skip`. */
function newest(dir, skip = []) {
  let max = mtime(dir);
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (skip.includes(entry.name)) continue;
    const path = join(dir, entry.name);
    max = Math.max(max, entry.isDirectory() ? newest(path) : mtime(path));
  }
  return max;
}

const inputs = Math.max(
  newest(join(core, "katex"), ["tests", "benches"]),
  newest(join(core, "edit"), ["tests"]),
  newest(join(core, "wasm")),
  mtime(join(core, "Cargo.toml")),
  mtime(join(core, "Cargo.lock")),
  mtime(fileURLToPath(import.meta.url)),
);
if (glue.every((f) => existsSync(join(pkg, f))) && mtime(wasm) >= inputs) {
  log("up to date");
  process.exit(0);
}

const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { encoding: "utf8", ...opts });

let targets = "";
try {
  targets = run("rustup", ["target", "list", "--installed"]);
} catch {
  fail("rustup is not on PATH — install Rust with rustup (https://rustup.rs)");
}
if (!targets.split("\n").includes(target)) fail(`the ${target} target is missing — run \`rustup target add ${target}\``);

const lock = readFileSync(join(core, "Cargo.lock"), "utf8");
const locked = lock.match(/^name = "wasm-bindgen"\nversion = "([^"]+)"/m)?.[1];
if (!locked) fail("math-core/Cargo.lock names no wasm-bindgen version");
const install = `cargo install wasm-bindgen-cli --version ${locked} --locked`;
let cli = "";
try {
  cli = run("wasm-bindgen", ["--version"]).trim();
} catch {
  fail(`the wasm-bindgen CLI is not on PATH — run \`${install}\``);
}
if (cli !== `wasm-bindgen ${locked}`) fail(`${cli} does not match the crate's wasm-bindgen ${locked} — run \`${install}\``);

const wasmOpt = join(app, "node_modules", ".bin", "wasm-opt");
if (!existsSync(wasmOpt)) fail("wasm-opt is missing — run `bun install` in app/");

const started = Date.now();
try {
  run("cargo", ["build", "--locked", "--profile", "wasm", "--target", target, "-p", "oculus-math"], {
    cwd: core,
    stdio: "inherit",
  });
  run("wasm-bindgen", ["--target", "web", "--out-dir", staging, "--out-name", "oculus_math", built], { stdio: "inherit" });
  const optimised = join(staging, "oculus_math_bg.opt.wasm");
  run(wasmOpt, ["-Oz", join(staging, "oculus_math_bg.wasm"), "-o", optimised], { stdio: "inherit" });

  // The .wasm goes last: its mtime is what marks the whole output current.
  mkdirSync(pkg, { recursive: true });
  for (const f of glue) copyFileSync(join(staging, f), join(pkg, f));
  renameSync(optimised, wasm);
} catch (e) {
  fail(`build failed: ${e.message}`);
}
const kb = Math.round(statSync(wasm).size / 1024);
log(`built ${relative(app, wasm)} in ${((Date.now() - started) / 1000).toFixed(0)}s, ${kb} KB`);
