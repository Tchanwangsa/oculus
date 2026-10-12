import { expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";

// A dependency-free Cargo crate makes this exercise the real config lookup
// and binary handling without building the desktop app or requiring sidecars.
test("CLI builds read the crate's Cargo config and reject a stale binary on failure", async () => {
  const fixture = mkdtempSync(join(tmpdir(), "oculus-cli-build-"));
  try {
    const script = join(fixture, "scripts", "runtime.mjs");
    const rust = join(fixture, "src-tauri");
    mkdirSync(dirname(script), { recursive: true });
    mkdirSync(join(rust, "src"), { recursive: true });
    mkdirSync(join(rust, ".cargo"));
    copyFileSync(new URL("./runtime.mjs", import.meta.url), script);
    writeFileSync(join(rust, "Cargo.toml"), `
[package]
name = "oculus-build-fixture"
version = "0.0.0"
edition = "2021"
[[bin]]
name = "oculus"
path = "src/main.rs"
`);
    writeFileSync(join(rust, ".cargo/config.toml"), `
[env]
OCULUS_BUILD_FIXTURE_CONFIG = { value = "crate-config", force = true }
`);
    const source = join(rust, "src/main.rs");
    writeFileSync(source, 'fn main() { println!("{}", env!("OCULUS_BUILD_FIXTURE_CONFIG")); }\n');
    const { buildCli, cliPath } = await import(pathToFileURL(script).href);
    const binary = buildCli("debug");
    expect(binary).toBe(cliPath("debug"));
    expect(execFileSync(binary, [], { encoding: "utf8" }).trim()).toBe("crate-config");

    // Make Cargo fail after a successful build: callers must never receive
    // the previous CLI as though it came from the now-invalid sources.
    writeFileSync(source, 'compile_error!("intentional fixture build failure");\n');
    expect(() => buildCli("debug")).toThrow();
    expect(existsSync(binary)).toBe(false);
  } finally {
    rmSync(fixture, { recursive: true, force: true });
  }
}, 60_000);
