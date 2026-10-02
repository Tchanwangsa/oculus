// Paths and cargo invocation shared by the development and bundle scripts.
import { execFileSync } from "node:child_process";
import { existsSync, rmSync } from "node:fs";
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

/** Remove the previous binary so cargo must uplift the newly linked CLI. */
export function buildCli(profile = "release") {
  const binary = cliPath(profile);
  rmSync(binary, { force: true });
  execFileSync("cargo", cliBuildArgs(profile), { stdio: "inherit" });
  if (!existsSync(binary)) throw new Error(`cargo reported success but ${binary} is not there`);
  return binary;
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
