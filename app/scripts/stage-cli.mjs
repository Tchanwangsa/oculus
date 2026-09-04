// Stage the `oculus` CLI as a Tauri sidecar.
//
// The macOS keep-alive LaunchAgent runs `oculus auth tick`, so the CLI has to
// be inside the installed app — a path under `target/` disappears on the first
// `cargo clean` and takes the keep-alive with it, silently.
//
// Tauri's bundler does copy *some* sibling cargo binaries into
// `Contents/MacOS/` (`retrieval_smoke` lands there), but not `oculus`, and the
// selection is undocumented and apparently name-dependent. `externalBin` is the
// documented mechanism and the one `ffmpeg` already uses, so the CLI takes the
// same route: build it, then stage it under the target-triple name the bundler
// expects.
//
// Run before `tauri build` (see beforeBuildCommand); `binaries/` is gitignored.

import { execFileSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const manifest = join(root, "src-tauri", "Cargo.toml");
const outDir = join(root, "src-tauri", "binaries");

function hostTriple() {
  try {
    const out = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
    const host = out.match(/^host:\s*(\S+)$/m)?.[1];
    if (host) return host;
  } catch {
    // rustc missing — fall through to a guess based on the current process.
  }
  const arch = process.arch === "arm64" ? "aarch64" : "x86_64";
  if (process.platform === "darwin") return `${arch}-apple-darwin`;
  if (process.platform === "win32") return `${arch}-pc-windows-msvc`;
  return `${arch}-unknown-linux-gnu`;
}

const triple = hostTriple();
const exe = process.platform === "win32" ? ".exe" : "";
const built = join(root, "src-tauri", "target", "release", `oculus${exe}`);
const dest = join(outDir, `oculus-${triple}${exe}`);

mkdirSync(outDir, { recursive: true });

// Chicken-and-egg: `tauri-build`'s build script checks that every `externalBin`
// resolves, and it runs for *every* binary in the crate — `oculus` included. So
// building the CLI requires the staged CLI to already be there. A placeholder
// satisfies the existence check for the one build that produces the real thing;
// after the first run the previous real binary is already in place and no
// placeholder is ever written.
let placeholder = false;
if (!existsSync(dest)) {
  writeFileSync(dest, "");
  chmodSync(dest, 0o755);
  placeholder = true;
}

try {
  // Cheap when already current: this is the same crate the app build compiles,
  // so the library is shared and only the CLI binary links.
  execFileSync(
    "cargo",
    ["build", "--release", "--manifest-path", manifest, "--bin", "oculus"],
    { stdio: "inherit" },
  );
} catch (e) {
  // Never leave an empty file behind claiming to be the CLI — the next bundle
  // would ship it, and a zero-byte sidecar fails only at keep-alive time, in
  // launchd, where nobody is looking.
  if (placeholder) rmSync(dest, { force: true });
  throw e;
}

const size = statSync(built).size;
if (size < 1_000_000) {
  if (placeholder) rmSync(dest, { force: true });
  throw new Error(`suspiciously small CLI build (${size} bytes) — refusing to stage it`);
}

copyFileSync(built, dest);
chmodSync(dest, 0o755);

console.log(`[cli] ${(size / 1e6).toFixed(1)} MB staged at ${dest}`);
