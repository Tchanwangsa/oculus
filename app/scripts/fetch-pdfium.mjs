#!/usr/bin/env node
// Downloads libpdfium into src-tauri/binaries/ so the page rasterizer
// (src-tauri/src/embed/raster.rs) has something to bind to at runtime.
// pdfium is a native C++ library with no crates.io source distribution — the
// Rust crate (`pdfium-render`) only speaks to a dylib that must already exist.
//
//   node scripts/fetch-pdfium.mjs          # host target only (what dev/build need)
//   node scripts/fetch-pdfium.mjs --all    # every target, for cross-building
//   node scripts/fetch-pdfium.mjs --force  # re-download even if present
//
// Builds come from bblanchon/pdfium-binaries' GitHub releases. The tag is
// pinned to the same Chromium revision `pdfium-render`'s default
// `pdfium_7881` feature binds against: that feature decides which symbols the
// crate looks up at load time, and a lib from a different revision is a
// missing-symbol failure at bind time rather than a compile error.
//
// Unlike the ffmpeg sidecar this is NOT an externalBin — it is a library, not
// an executable. It reaches the bundle through `bundle.macOS.frameworks` in
// tauri.conf.json, which copies it to Contents/Frameworks/.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

// Keep in lockstep with the `pdfium_*` feature on pdfium-render in Cargo.toml.
const RELEASE = "chromium/7881";
const BASE = `https://github.com/bblanchon/pdfium-binaries/releases/download/${RELEASE}`;

/** Rust target triple → [release asset, path inside the archive, installed name]. */
const ASSETS = {
  "aarch64-apple-darwin": ["pdfium-mac-arm64.tgz", "lib/libpdfium.dylib", "libpdfium.dylib"],
  "x86_64-apple-darwin": ["pdfium-mac-x64.tgz", "lib/libpdfium.dylib", "libpdfium.dylib"],
  "x86_64-pc-windows-msvc": ["pdfium-win-x64.tgz", "bin/pdfium.dll", "pdfium.dll"],
  "aarch64-pc-windows-msvc": ["pdfium-win-arm64.tgz", "bin/pdfium.dll", "pdfium.dll"],
  "x86_64-unknown-linux-gnu": ["pdfium-linux-x64.tgz", "lib/libpdfium.so", "libpdfium.so"],
  "aarch64-unknown-linux-gnu": ["pdfium-linux-arm64.tgz", "lib/libpdfium.so", "libpdfium.so"],
};

const outDir = join(dirname(dirname(fileURLToPath(import.meta.url))), "src-tauri", "binaries");

/** Match the exact rustc host triple, the same way fetch-ffmpeg.mjs does. */
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

async function fetchOne(triple, force) {
  const entry = ASSETS[triple];
  if (!entry) throw new Error(`no pdfium build mapped for target ${triple}`);
  const [asset, inner, name] = entry;

  // Cross-builds would collide on one name, so only the host target keeps the
  // bare platform filename the Rust loader looks for.
  const dest = join(outDir, triple === hostTriple() ? name : `${triple}-${name}`);

  // A truncated download from an earlier interrupted run must not count as done.
  if (!force && existsSync(dest) && statSync(dest).size > 1_000_000) {
    console.log(`[pdfium] already present: ${dest}`);
    return dest;
  }

  console.log(`[pdfium] downloading ${asset} → ${dest}`);
  const res = await fetch(`${BASE}/${asset}`, { redirect: "follow" });
  if (!res.ok) throw new Error(`${res.status} ${res.statusText} fetching ${asset}`);

  const buf = Buffer.from(await res.arrayBuffer());
  if (buf.length < 1_000_000) throw new Error(`suspiciously small download (${buf.length} bytes)`);

  mkdirSync(outDir, { recursive: true });
  const archive = join(tmpdir(), `oculus-${asset}`);
  writeFileSync(archive, buf);
  try {
    // `tar` ships with macOS, Linux and Windows 10+; node has no unpacker.
    execFileSync("tar", ["-xzf", archive, "-C", tmpdir(), inner], { stdio: "inherit" });
    const tmp = `${dest}.part`;
    rmSync(tmp, { force: true });
    renameSync(join(tmpdir(), inner), tmp);
    rmSync(dest, { force: true });
    renameSync(tmp, dest);
  } finally {
    rmSync(archive, { force: true });
  }

  console.log(`[pdfium] ${(statSync(dest).size / 1e6).toFixed(1)} MB ready (${RELEASE})`);
  return dest;
}

const force = process.argv.includes("--force");
const targets = process.argv.includes("--all") ? Object.keys(ASSETS) : [hostTriple()];

for (const triple of targets) {
  await fetchOne(triple, force);
}
