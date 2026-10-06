// Build whisper.cpp's whisper-cli from a pinned release into a Tauri sidecar.
// macOS only; needs cmake and Xcode or its command-line tools. Skipped while
// the installed binary reports the pinned version; --force rebuilds.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { availableParallelism } from "node:os";
import {
  chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, renameSync, rmSync, writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { app, binaries, hostTriple } from "./runtime.mjs";

const VERSION = "1.9.4";
const TARBALL = `https://github.com/ggml-org/whisper.cpp/archive/refs/tags/v${VERSION}.tar.gz`;
const SHA256 = "57e280cee375ab02425b806ad5146b99f6eb9357e3c2b31357c8a6af2e2e44ae";

// Upstream's macOS xcframework floor (build-xcframework.sh); ggml-metal gates
// its newer paths behind @available, so older Metal GPUs still run.
const MIN_MACOS = "13.3";

const log = (msg) => console.log(`[whisper] ${msg}`);

if (process.platform !== "darwin") {
  log("whisper.cpp is only packaged for macOS — nothing built");
  process.exit(0);
}

const triple = hostTriple();
const dest = join(binaries, `whisper-cli-${triple}`);

function installedVersion() {
  try {
    return execFileSync(dest, ["--version"], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
  } catch {
    return null;
  }
}

if (!process.argv.includes("--force") && installedVersion() === `whisper.cpp version: ${VERSION}`) {
  log(`${dest} is current (${VERSION})`);
  process.exit(0);
}

try {
  execFileSync("cmake", ["--version"], { stdio: "ignore" });
} catch {
  console.error("[whisper] cmake is not on PATH — install it with `brew install cmake`");
  process.exit(1);
}

// node_modules/.cache is gitignored and outside src-tauri, which tauri dev watches.
const cache = join(app, "node_modules", ".cache", "whisper.cpp");
const source = join(cache, `whisper.cpp-${VERSION}`);
const build = join(cache, `build-${VERSION}-${triple}`);

async function fetchSource() {
  if (existsSync(join(source, "CMakeLists.txt"))) return;
  log(`downloading ${TARBALL}`);
  const res = await fetch(TARBALL, { redirect: "follow" });
  if (!res.ok) throw new Error(`${res.status} ${res.statusText} fetching ${TARBALL}`);
  const buf = Buffer.from(await res.arrayBuffer());
  const sha = createHash("sha256").update(buf).digest("hex");
  if (sha !== SHA256) throw new Error(`whisper.cpp v${VERSION} tarball hash ${sha}, expected ${SHA256}`);

  mkdirSync(cache, { recursive: true });
  const scratch = mkdtempSync(join(cache, "extract-"));
  try {
    const archive = join(scratch, "src.tar.gz");
    writeFileSync(archive, buf);
    execFileSync("tar", ["-xzf", archive, "-C", scratch], { stdio: "inherit" });
    rmSync(source, { recursive: true, force: true });
    renameSync(join(scratch, `whisper.cpp-${VERSION}`), source);
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
}

// Every non-system dylib would be missing from the bundle, so refuse it here.
function assertSystemLinkage(binary) {
  const deps = execFileSync("otool", ["-L", binary], { encoding: "utf8" })
    .split("\n").slice(1).map((l) => l.trim().split(" ")[0]).filter(Boolean);
  const foreign = deps.filter((d) => !d.startsWith("/usr/lib/") && !d.startsWith("/System/Library/"));
  if (foreign.length) throw new Error(`whisper-cli links non-system libraries: ${foreign.join(", ")}`);
}

const started = Date.now();
try {
  await fetchSource();
  const arch = triple.startsWith("aarch64") ? "arm64" : "x86_64";
  execFileSync("cmake", [
    "-S", source, "-B", build,
    "-DCMAKE_BUILD_TYPE=Release",
    `-DCMAKE_OSX_ARCHITECTURES=${arch}`,
    `-DCMAKE_OSX_DEPLOYMENT_TARGET=${MIN_MACOS}`,
    "-DWHISPER_BUILD_IS_DEV=OFF", // so --version prints the bare tag the skip check reads
    // Static, with the Metal shaders compiled in: one file and system frameworks only.
    "-DBUILD_SHARED_LIBS=OFF",
    "-DGGML_METAL=ON",
    "-DGGML_METAL_EMBED_LIBRARY=ON",
    // Generic CPU code, not tuned to the machine that happens to build it.
    "-DGGML_NATIVE=OFF",
    // Apple clang has no OpenMP; a Homebrew libomp would otherwise be linked in.
    "-DGGML_OPENMP=OFF",
    "-DWHISPER_BUILD_EXAMPLES=ON",
    "-DWHISPER_BUILD_TESTS=OFF",
    "-DWHISPER_BUILD_SERVER=OFF",
    "-DWHISPER_SDL2=OFF",
    "-DWHISPER_CURL=OFF",
  ], { stdio: "inherit" });
  execFileSync("cmake", [
    "--build", build, "--config", "Release", "--target", "whisper-cli",
    "--parallel", String(availableParallelism()),
  ], { stdio: "inherit" });

  const built = join(build, "bin", "whisper-cli");
  assertSystemLinkage(built);
  mkdirSync(binaries, { recursive: true });
  const partial = `${dest}.part`;
  try {
    copyFileSync(built, partial);
    chmodSync(partial, 0o755);
    renameSync(partial, dest);
  } finally {
    rmSync(partial, { force: true });
  }
} catch (e) {
  console.error(`[whisper] build failed: ${e.message}`);
  process.exit(1);
}
log(`built ${dest} (${VERSION}) in ${((Date.now() - started) / 1000).toFixed(0)}s`);
