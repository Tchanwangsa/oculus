import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { encodeQr } from "@/lib/qr";
import { LINKS } from "./links";
import { qrPng } from "./png";

// End to end: each code is drawn to a PNG and read back by macOS's own QR
// detector (CoreImage, through decode.swift). Skipped where there is no swift.
const swift = process.platform === "darwin" && spawnSync("swift", ["--version"], { encoding: "utf8" }).status === 0;

test.skipIf(!swift)("CoreImage decodes every link the encoder draws", () => {
  const dir = mkdtempSync(join(tmpdir(), "oculus-qr-"));
  try {
    const versions = new Set<number>();
    const paths = LINKS.map((link, i) => {
      const code = encodeQr(link);
      versions.add(code.version);
      const path = join(dir, `${i}.png`);
      writeFileSync(path, qrPng(code.modules));
      return path;
    });
    const run = spawnSync("swift", [join(import.meta.dir, "decode.swift"), ...paths], { encoding: "utf8", timeout: 120_000 });
    expect(run.stderr.trim() && run.status !== 0 ? run.stderr : "").toBe("");
    expect(run.status).toBe(0);
    expect(JSON.parse(run.stdout.trim())).toEqual([...LINKS]);
    expect(versions.size).toBeGreaterThanOrEqual(6);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}, 180_000);
