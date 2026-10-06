// Compile the on-device speech helper (src-tauri/speech/main.swift) into a
// Tauri sidecar. macOS only, needs swiftc (Xcode or its command-line tools);
// skipped while the binary is newer than its source.
import { execFileSync } from "node:child_process";
import { chmodSync, mkdirSync, renameSync, rmSync, statSync } from "node:fs";
import { join } from "node:path";
import { binaries, hostTriple, rust } from "./runtime.mjs";

// Tauri 2's floor. The helper itself only recognises speech on macOS 26 and
// answers "unavailable" below that, so it must still launch there.
const MIN_MACOS = "10.15";

const log = (msg) => console.log(`[speech] ${msg}`);

if (process.platform !== "darwin") {
  log("on-device speech is macOS only — nothing built");
  process.exit(0);
}

const source = join(rust, "speech", "main.swift");
const triple = hostTriple();
const dest = join(binaries, `apple-speech-${triple}`);

function mtime(p) {
  try {
    return statSync(p).mtimeMs;
  } catch {
    return 0;
  }
}

if (mtime(dest) > mtime(source)) {
  log(`${dest} is current`);
  process.exit(0);
}

const arch = triple.startsWith("aarch64") ? "arm64" : "x86_64";
const partial = `${dest}.part`;
mkdirSync(binaries, { recursive: true });
try {
  execFileSync(
    "swiftc",
    ["-O", "-parse-as-library", "-target", `${arch}-apple-macos${MIN_MACOS}`, "-o", partial, source],
    { stdio: "inherit" },
  );
} catch (e) {
  rmSync(partial, { force: true });
  console.error(`[speech] swiftc failed — install Xcode or its command-line tools: ${e.message}`);
  process.exit(1);
}
chmodSync(partial, 0o755);
renameSync(partial, dest);
log(`built ${dest}`);
