// Fetch ffmpeg's static GPL build as a Tauri sidecar. --all fetches every
// target; --force replaces cached downloads.
import { chmodSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { binaries } from "./runtime.mjs";
import { fetchNative } from "./native-binary.mjs";

const BASE = "https://github.com/eugeneware/ffmpeg-static/releases/download/b6.0";
const ASSETS = {
  "aarch64-apple-darwin": "ffmpeg-darwin-arm64",
  "x86_64-apple-darwin": "ffmpeg-darwin-x64",
  "x86_64-pc-windows-msvc": "ffmpeg-win32-x64",
  "aarch64-pc-windows-msvc": "ffmpeg-win32-x64", // x64 runs under emulation
  "x86_64-unknown-linux-gnu": "ffmpeg-linux-x64",
  "aarch64-unknown-linux-gnu": "ffmpeg-linux-arm64",
};

await fetchNative("ffmpeg", ASSETS, (asset, triple) => ({
  asset,
  url: `${BASE}/${asset}`,
  dest: join(binaries, `ffmpeg-${triple}${triple.includes("windows") ? ".exe" : ""}`),
  install(buf, partial) {
    writeFileSync(partial, buf);
    chmodSync(partial, 0o755);
  },
}));
