// Cached downloads are installed through a sibling .part file. Each fetcher
// supplies only the asset mapping and how to unpack its payload.
import { randomUUID } from "node:crypto";
import { existsSync, mkdirSync, renameSync, rmSync, statSync } from "node:fs";
import { dirname } from "node:path";
import { hostTriple } from "./runtime.mjs";

const MIN_BYTES = 1_000_000;

export async function fetchNative(label, assets, describe) {
  const host = hostTriple();
  const force = process.argv.includes("--force");
  const targets = process.argv.includes("--all") ? Object.keys(assets) : [host];
  for (const triple of targets) {
    if (!assets[triple]) throw new Error(`no ${label} build mapped for target ${triple}`);
    const { asset, dest, url, install, suffix = "" } = describe(assets[triple], triple, host);
    if (!force && existsSync(dest) && statSync(dest).size > MIN_BYTES) {
      console.log(`[${label}] already present: ${dest}`);
      continue;
    }
    console.log(`[${label}] downloading ${asset} → ${dest}`);
    const res = await fetch(url, { redirect: "follow" });
    if (!res.ok) throw new Error(`${res.status} ${res.statusText} fetching ${asset}`);
    const buf = Buffer.from(await res.arrayBuffer());
    if (buf.length < MIN_BYTES) throw new Error(`suspiciously small download (${buf.length} bytes)`);

    mkdirSync(dirname(dest), { recursive: true });
    const partial = `${dest}.${process.pid}-${randomUUID()}.part`;
    try {
      await install(buf, partial);
      // Rename replaces the previous file atomically. A failed installation
      // or rename leaves the cached binary intact.
      renameSync(partial, dest);
    } finally {
      rmSync(partial, { force: true });
    }
    console.log(`[${label}] ${(statSync(dest).size / 1e6).toFixed(1)} MB ready${suffix}`);
  }
}
