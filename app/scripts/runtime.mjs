// Paths and cargo invocation shared by the development and bundle scripts.
import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync } from "node:fs";
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

/** keyd's helper app, the name macOS shows for it (`keyd_core::paths::HELPER`). */
export const keydHelperApp = "Oculus Helper.app";

/**
 * keyd's executable inside a helper app, named like the app
 * (`keyd_core::paths::helper_program`). The release bundle copies the staged
 * helper to `Contents/Helpers/` (`bundle.macOS.files` in tauri.macos.conf.json).
 */
export const helperProgram = (helper) => join(helper, "Contents", "MacOS", `Oculus Helper${exe}`);

/**
 * Copy a signed helper app to `dir` (`binaries/` unless a script is pointed
 * elsewhere) as a new tree renamed into place: a signed binary rewritten in
 * place leaves the kernel's cached signature stale.
 */
export function stageKeyd(helper, dir = binaries) {
  const dest = join(dir, keydHelperApp);
  const partial = join(dir, `.${keydHelperApp}.${process.pid}.part`);
  mkdirSync(dir, { recursive: true });
  rmSync(partial, { recursive: true, force: true });
  try {
    cpSync(helper, partial, { recursive: true });
    rmSync(dest, { recursive: true, force: true });
    renameSync(partial, dest);
  } finally {
    rmSync(partial, { recursive: true, force: true });
  }
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
