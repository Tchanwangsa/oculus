// Paths and cargo invocation shared by the development and bundle scripts.
import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const app = dirname(dirname(fileURLToPath(import.meta.url)));
export const rust = join(app, "src-tauri");
export const manifest = join(rust, "Cargo.toml");
export const binaries = join(rust, "binaries");
export const exe = process.platform === "win32" ? ".exe" : "";
export const cliPath = (profile = "release") => join(rust, "target", profile, `oculus${exe}`);
export const cliBuildArgs = (profile = "release") => [
  "build", ...(profile === "release" ? ["--release"] : []),
  "--manifest-path", manifest, "--bin", "oculus",
];

/** Remove the previous binary so a build that produces none cannot pass on it. */
export function buildCli(profile = "release") {
  const binary = cliPath(profile);
  rmSync(binary, { force: true });
  execFileSync("cargo", cliBuildArgs(profile), { stdio: "inherit" });
  if (!existsSync(binary)) throw new Error(`cargo reported success but ${binary} is not there`);
  return binary;
}

/** The `externalBin` copy of the CLI under `binaries/`. */
export const cliSidecar = () => join(binaries, `oculus-${hostTriple()}${exe}`);

/**
 * Copy a built CLI over the sidecar. tauri-build copies the sidecar to
 * `target/<profile>/oculus` whenever the app's build script runs, so a stale
 * sidecar overwrites the CLI that was just built.
 */
export function stageCli(binary) {
  const dest = cliSidecar();
  mkdirSync(binaries, { recursive: true });
  // The build script watches the sidecar's mtime, so an identical copy would
  // still recompile the crate.
  if (existsSync(dest) && readFileSync(dest).equals(readFileSync(binary))) return;
  rmSync(dest, { force: true });
  copyFileSync(binary, dest);
  chmodSync(dest, 0o755);
}

/** The `externalBin` copy of keyd under `dir` (`binaries/` unless a script is pointed elsewhere). */
export const keydSidecar = (dir = binaries) => join(dir, `oculus-keyd-${hostTriple()}${exe}`);

/**
 * Copy a signed keyd over its sidecar, unless the sidecar already holds the
 * same bytes: a reproducible build then leaves the file alone, so tauri-build
 * (which reruns when a sidecar changes) has nothing to redo.
 */
export function stageKeyd(binary, dir = binaries) {
  const dest = keydSidecar(dir);
  mkdirSync(dir, { recursive: true });
  if (existsSync(dest) && readFileSync(dest).equals(readFileSync(binary))) return dest;
  // A new file, never an overwrite: a signed binary rewritten in place leaves
  // the kernel's cached signature stale.
  rmSync(dest, { force: true });
  copyFileSync(binary, dest);
  chmodSync(dest, 0o755);
  return dest;
}

/** Tauri sidecars must use rustc's exact host triple. */
export function hostTriple() {
  try {
    const host = execFileSync("rustc", ["-vV"], { encoding: "utf8" }).match(/^host:\s*(\S+)$/m)?.[1];
    if (host) return host;
  } catch {
    // rustc missing: infer the common target for this process.
  }
  const arch = process.arch === "arm64" ? "aarch64" : "x86_64";
  if (process.platform === "darwin") return `${arch}-apple-darwin`;
  if (process.platform === "win32") return `${arch}-pc-windows-msvc`;
  return `${arch}-unknown-linux-gnu`;
}
