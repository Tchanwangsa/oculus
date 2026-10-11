// Build the editor core's WebAssembly bridge (editor-core/wasm) and its JS glue
// into src/components/documents/editor/shadow/pkg/ (gitignored). Needs the
// wasm32-unknown-unknown target and the wasm-bindgen CLI at exactly the version
// editor-core/wasm/Cargo.toml pins; without them it says what to install and
// fails, or with --optional skips.
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, rmSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { app } from "./runtime.mjs";

const TARGET = "wasm32-unknown-unknown";
const PROFILE = "wasm"; // editor-core/Cargo.toml: release with LTO
const crate = join(app, "editor-core");
const out = join(app, "src", "components", "documents", "editor", "shadow", "pkg");

const optional = process.argv.includes("--optional");
const log = (msg) => console.log(`[editor-wasm] ${msg}`);

function missing(what) {
  if (optional) {
    console.warn(`[editor-wasm] ${what} — editor shadow mode skipped`);
    process.exit(0);
  }
  console.error(`[editor-wasm] ${what}`);
  process.exit(1);
}

function mtime(path) {
  try {
    return statSync(path).mtimeMs;
  } catch {
    return 0;
  }
}

function stdout(cmd, args) {
  try {
    return execFileSync(cmd, args, { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] });
  } catch {
    return null;
  }
}

// The CLI must match the crate exactly: its glue is generated against the
// crate's own wasm-bindgen.
const manifest = readFileSync(join(crate, "wasm", "Cargo.toml"), "utf8");
const pinned = manifest.match(/^wasm-bindgen\s*=\s*"=([^"]+)"/m)?.[1];
if (!pinned) {
  console.error("[editor-wasm] editor-core/wasm/Cargo.toml does not pin wasm-bindgen with \"=x.y.z\"");
  process.exit(1);
}

const targets = stdout("rustup", ["target", "list", "--installed"]);
if (!targets?.split("\n").includes(TARGET)) missing(`no ${TARGET} target — install it with \`rustup target add ${TARGET}\``);
const cli = stdout("wasm-bindgen", ["--version"])?.trim();
if (cli !== `wasm-bindgen ${pinned}`) {
  missing(`${cli ? `${cli} is installed` : "wasm-bindgen is not on PATH"} — install ${pinned} with \`cargo install wasm-bindgen-cli --version ${pinned} --locked\``);
}

const started = Date.now();
try {
  execFileSync(
    "cargo",
    ["build", "--locked", "-p", "oculus-editor-core-wasm", "--profile", PROFILE, "--target", TARGET],
    { cwd: crate, stdio: "inherit" },
  );
  const targetDir = process.env.CARGO_TARGET_DIR ?? join(crate, "target");
  const wasm = join(targetDir, TARGET, PROFILE, "oculus_editor_core_wasm.wasm");
  if (!existsSync(wasm)) throw new Error(`cargo reported success but ${wasm} is not there`);
  // Cargo leaves the .wasm alone when nothing changed; the glue is then
  // current too, and rewriting it would only wake a running Vite.
  const outputs = ["oculus_editor_core_wasm.js", "oculus_editor_core_wasm_bg.wasm"].map((f) => join(out, f));
  const newest = Math.max(mtime(wasm), mtime(fileURLToPath(import.meta.url)));
  if (outputs.every((f) => mtime(f) >= newest)) {
    log(`up to date (${out})`);
    process.exit(0);
  }
  rmSync(out, { recursive: true, force: true });
  execFileSync("wasm-bindgen", ["--target", "web", "--weak-refs", "--out-dir", out, wasm], { stdio: "inherit" });
  const size = statSync(join(out, "oculus_editor_core_wasm_bg.wasm")).size;
  log(`built ${out} (${(size / 1024).toFixed(0)} KiB wasm) in ${((Date.now() - started) / 1000).toFixed(1)}s`);
} catch (e) {
  console.error(`[editor-wasm] build failed: ${e.message}`);
  process.exit(1);
}
