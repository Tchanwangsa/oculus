// Fetch ffmpeg as a Tauri sidecar. Every target gets our own LGPL build, which
// .github/workflows/ffmpeg.yml publishes with its exact sources as a release
// of this repository, pinned by SHA-256. Intel Macs and Linux have no build.
// --all fetches every target; --force replaces cached downloads.
import { chmodSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { binaries } from "./runtime.mjs";
import { fetchNative } from "./native-binary.mjs";

// The tag and hashes the ffmpeg workflow's run summary reports; they change
// with the notices in src-tauri/ffmpeg-notices/.
const OCULUS = "https://github.com/Tchanwangsa/oculus/releases/download/ffmpeg-8.1.3-oculus2";
const MACOS = {
  url: `${OCULUS}/ffmpeg-macos-arm64`,
  sha256: "202a0f68c33ca4ce89dd35ae80542484e9cfadf3363a7dde2a49616f25172906",
};
const WINDOWS = {
  url: `${OCULUS}/ffmpeg-windows-x64.exe`,
  sha256: "2f63039b98cb1afbb5d071933195f3e2e83dac1b80fd1d39b4407a534d1aaecb",
};
const ASSETS = {
  "aarch64-apple-darwin": MACOS,
  "x86_64-pc-windows-msvc": WINDOWS,
  "aarch64-pc-windows-msvc": WINDOWS, // x64 runs under emulation
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
