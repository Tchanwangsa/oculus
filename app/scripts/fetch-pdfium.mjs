// libpdfium is a library, bundled through bundle.macOS.frameworks rather
// than externalBin. --all fetches every target; --force replaces downloads.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { binaries } from "./runtime.mjs";
import { fetchNative } from "./native-binary.mjs";

// Keep in lockstep with the pdfium_* feature on pdfium-render in Cargo.toml:
// a mismatched revision fails at bind time, even when the crate compiles.
const RELEASE = "chromium/7881";
const BASE = `https://github.com/bblanchon/pdfium-binaries/releases/download/${RELEASE}`;
const ASSETS = {
  "aarch64-apple-darwin": ["pdfium-mac-arm64.tgz", "lib/libpdfium.dylib", "libpdfium.dylib"],
  "x86_64-apple-darwin": ["pdfium-mac-x64.tgz", "lib/libpdfium.dylib", "libpdfium.dylib"],
  "x86_64-pc-windows-msvc": ["pdfium-win-x64.tgz", "bin/pdfium.dll", "pdfium.dll"],
  "aarch64-pc-windows-msvc": ["pdfium-win-arm64.tgz", "bin/pdfium.dll", "pdfium.dll"],
  "x86_64-unknown-linux-gnu": ["pdfium-linux-x64.tgz", "lib/libpdfium.so", "libpdfium.so"],
  "aarch64-unknown-linux-gnu": ["pdfium-linux-arm64.tgz", "lib/libpdfium.so", "libpdfium.so"],
};

await fetchNative("pdfium", ASSETS, ([asset, inner, name], triple, host) => ({
  asset,
  url: `${BASE}/${asset}`,
  // Only the host uses the bare filename the loader expects.
  dest: join(binaries, triple === host ? name : `${triple}-${name}`),
  suffix: ` (${RELEASE})`,
  install(buf, partial) {
    const scratch = mkdtempSync(join(tmpdir(), "oculus-pdfium-"));
    try {
      const archive = join(scratch, asset);
      writeFileSync(archive, buf);
      // tar ships with macOS, Linux and Windows 10+.
      execFileSync("tar", ["-xzf", archive, "-C", scratch, inner], { stdio: "inherit" });
      copyFileSync(join(scratch, inner), partial);
    } finally {
      rmSync(scratch, { recursive: true, force: true });
    }
  },
}));
