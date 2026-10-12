// Cached downloads are installed through a sibling .part file. Each fetcher
// supplies only the asset mapping and how to unpack its payload, plus an
// optional SHA-256 the download must match; a cached copy with another hash is
// downloaded again.
import { createHash, randomUUID } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, statSync } from "node:fs";
import { dirname } from "node:path";
import { hostTriple } from "./runtime.mjs";

const MIN_BYTES = 1_000_000;

export async function fetchNative(label, assets, describe) {
  const host = hostTriple();
  const force = process.argv.includes("--force");
  const targets = process.argv.includes("--all") ? Object.keys(assets) : [host];
  for (const triple of targets) {
    if (!assets[triple]) throw new Error(`no ${label} build mapped for target ${triple}`);
    const { asset, dest, url, install, sha256, suffix = "" } = describe(assets[triple], triple, host);
    if (!force && existsSync(dest) && statSync(dest).size > MIN_BYTES
      && (!sha256 || hash(readFileSync(dest)) === sha256)) {
      console.log(`[${label}] already present: ${dest}`);
      continue;
    }
    console.log(`[${label}] downloading ${asset} → ${dest}`);
    const res = await fetch(url, { redirect: "follow" });
    if (!res.ok) throw new Error(`${res.status} ${res.statusText} fetching ${asset}`);
    const buf = Buffer.from(await res.arrayBuffer());
    if (buf.length < MIN_BYTES) throw new Error(`suspiciously small download (${buf.length} bytes)`);
    if (sha256 && hash(buf) !== sha256) throw new Error(`${asset} has SHA-256 ${hash(buf)}, expected ${sha256}`);

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

function hash(buf) {
  return createHash("sha256").update(buf).digest("hex");
}
