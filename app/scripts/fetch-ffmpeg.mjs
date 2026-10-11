// Fetch ffmpeg as a Tauri sidecar. Windows gets our own LGPL build, which
// .github/workflows/ffmpeg.yml publishes with its exact sources as a release
// of this repository, pinned by SHA-256; the other targets get ffmpeg-static's
// GPL builds. --all fetches every target; --force replaces cached downloads.
import { chmodSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { binaries } from "./runtime.mjs";
import { fetchNative } from "./native-binary.mjs";

const STATIC = "https://github.com/eugeneware/ffmpeg-static/releases/download/b6.0";
// The tag and hash the ffmpeg workflow's run summary reports; they change with
// the notices in src-tauri/windows/distribution/.
const OCULUS = "https://github.com/Tchanwangsa/oculus/releases/download/ffmpeg-8.1.3-oculus1";
const WINDOWS = {
  url: `${OCULUS}/ffmpeg-windows-x64.exe`,
  sha256: "941a81e7c8878bf56990c104e85247656d6cee9c9ede3ab2927e6933ed33d1b7",
};
const ASSETS = {
  "aarch64-apple-darwin": { url: `${STATIC}/ffmpeg-darwin-arm64` },
  "x86_64-apple-darwin": { url: `${STATIC}/ffmpeg-darwin-x64` },
  "x86_64-pc-windows-msvc": WINDOWS,
  "aarch64-pc-windows-msvc": WINDOWS, // x64 runs under emulation
  "x86_64-unknown-linux-gnu": { url: `${STATIC}/ffmpeg-linux-x64` },
  "aarch64-unknown-linux-gnu": { url: `${STATIC}/ffmpeg-linux-arm64` },
};

await fetchNative("ffmpeg", ASSETS, ({ url, sha256 }, triple) => ({
  asset: url.slice(url.lastIndexOf("/") + 1),
  url,
  sha256,
  dest: join(binaries, `ffmpeg-${triple}${triple.includes("windows") ? ".exe" : ""}`),
  install(buf, partial) {
    writeFileSync(partial, buf);
    chmodSync(partial, 0o755);
  },
}));
