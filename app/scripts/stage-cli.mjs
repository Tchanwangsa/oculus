// Stage the CLI as a Tauri sidecar for the keep-alive LaunchAgent.
import { chmodSync, copyFileSync, existsSync, mkdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { binaries, buildCli, cliPath, exe, hostTriple } from "./runtime.mjs";

const built = cliPath();
const dest = join(binaries, `oculus-${hostTriple()}${exe}`);

mkdirSync(binaries, { recursive: true });

// tauri-build checks externalBin even while building the CLI itself. A
// placeholder breaks that cycle and is removed if the real build fails.
let placeholder = false;
if (!existsSync(dest)) {
  writeFileSync(dest, "");
  chmodSync(dest, 0o755);
  placeholder = true;
}

try {
  buildCli();
} catch (e) {
  // An empty sidecar must never survive a failed build.
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
