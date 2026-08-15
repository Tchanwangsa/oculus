#!/usr/bin/env node
// Downloads a static ffmpeg into src-tauri/binaries/ so Tauri can bundle it as
// an externalBin sidecar. Without this the trim step depends on whatever ffmpeg
// happens to be on the user's PATH — which on a fresh machine is nothing.
//
//   node scripts/fetch-ffmpeg.mjs          # host target only (what dev/build need)
//   node scripts/fetch-ffmpeg.mjs --all    # every target, for cross-building
//   node scripts/fetch-ffmpeg.mjs --force  # re-download even if present
//
// Binaries come from eugeneware/ffmpeg-static's GitHub releases: single-file,
// statically linked, no archive to unpack. They are GPL builds.

import { execFileSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const RELEASE = "b6.0";
const BASE = `https://github.com/eugeneware/ffmpeg-static/releases/download/${RELEASE}`;

/** Rust target triple → release asset name. */
const ASSETS = {
  "aarch64-apple-darwin": "ffmpeg-darwin-arm64",
  "x86_64-apple-darwin": "ffmpeg-darwin-x64",
  "x86_64-pc-windows-msvc": "ffmpeg-win32-x64",
  "aarch64-pc-windows-msvc": "ffmpeg-win32-x64", // no arm64 build; x64 runs under emulation
  "x86_64-unknown-linux-gnu": "ffmpeg-linux-x64",
  "aarch64-unknown-linux-gnu": "ffmpeg-linux-arm64",
};

const outDir = join(dirname(dirname(fileURLToPath(import.meta.url))), "src-tauri", "binaries");

/** Tauri matches the sidecar suffix against the exact rustc host triple. */
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
  const asset = ASSETS[triple];
  if (!asset) throw new Error(`no static ffmpeg mapped for target ${triple}`);

  const ext = triple.includes("windows") ? ".exe" : "";
  const dest = join(outDir, `ffmpeg-${triple}${ext}`);

  // A truncated download from an earlier interrupted run must not count as done.
  if (!force && existsSync(dest) && statSync(dest).size > 1_000_000) {
    console.log(`[ffmpeg] already present: ${dest}`);
    return dest;
  }

  console.log(`[ffmpeg] downloading ${asset} → ${dest}`);
  const res = await fetch(`${BASE}/${asset}`, { redirect: "follow" });
  if (!res.ok) throw new Error(`${res.status} ${res.statusText} fetching ${asset}`);

  const buf = Buffer.from(await res.arrayBuffer());
  if (buf.length < 1_000_000) throw new Error(`suspiciously small download (${buf.length} bytes)`);

  mkdirSync(outDir, { recursive: true });
  const tmp = `${dest}.part`;
  writeFileSync(tmp, buf);
  chmodSync(tmp, 0o755);
  rmSync(dest, { force: true });
  renameSync(tmp, dest);

  console.log(`[ffmpeg] ${(buf.length / 1e6).toFixed(1)} MB ready`);
  return dest;
}

const force = process.argv.includes("--force");
const targets = process.argv.includes("--all") ? Object.keys(ASSETS) : [hostTriple()];

for (const triple of targets) {
  await fetchOne(triple, force);
}
