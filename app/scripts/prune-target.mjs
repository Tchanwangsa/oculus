// Deletes the build output under target/debug that cargo will never reuse.
// Cargo keeps a copy of each crate per dependency set, feature set and profile,
// named by a hash, and never deletes a superseded one: every Cargo.lock change
// leaves a stale copy of what it touched, and rustc's incremental cache does the
// same for our own crates.
//
// A copy goes when the dev build (the preflight's CLI build, which `tauri dev`
// shares) did not use it and nothing has rewritten it for PRUNE_DAYS. Cargo
// reports every unit it used, fresh or rebuilt, so the dev loop never loses
// anything. A copy only `cargo test` or clippy uses goes once it is that old,
// and is rebuilt on their next run, mostly from sccache. File access times
// would be the better signal, but APFS does not keep them.
//
// The pure half is exported for prune-target.test.mjs. Fail-open: any error
// logs one line and the preflight carries on.
import { lstatSync, readdirSync, rmSync } from "node:fs";
import { join } from "node:path";

export const PRUNE_DAYS = 14;
const DAY_MS = 24 * 60 * 60 * 1000;

/** The 16-hex unit hash in a `deps/` file or `build/` directory name, or null. */
export function unitHash(name) {
  return name.match(/-([0-9a-f]{16})(?=[.]|$)/)?.[1] ?? null;
}

/** The unit hashes a build used, from cargo's `--message-format=json` stdout. */
export function usedHashes(stdout) {
  const used = new Set();
  const add = (path) => {
    for (const part of path.split(/[\\/]/)) {
      const hash = unitHash(part);
      if (hash) used.add(hash);
    }
  };
  for (const line of stdout.split("\n")) {
    if (!line.startsWith("{")) continue;
    let msg;
    try {
      msg = JSON.parse(line);
    } catch {
      continue;
    }
    if (msg.reason === "compiler-artifact") {
      for (const path of msg.filenames ?? []) add(path);
      if (msg.executable) add(msg.executable);
    } else if (msg.reason === "build-script-executed" && msg.out_dir) {
      add(msg.out_dir);
    }
  }
  return used;
}

/**
 * The entries to delete: `[{ path, hash, crate, mtimeMs, bytes }]` older than
 * `days` whose hash the dev build did not use. Incremental caches have no unit
 * hash, so they go on age, except the newest of each crate, which a dev build
 * with no Rust edits for that long still uses.
 */
export function planPrune(entries, used, now, days = PRUNE_DAYS) {
  const cutoff = now - days * DAY_MS;
  const newest = new Map();
  for (const e of entries) {
    if (e.hash == null && !(newest.get(e.crate)?.mtimeMs >= e.mtimeMs)) newest.set(e.crate, e);
  }
  return entries.filter((e) =>
    e.mtimeMs < cutoff && (e.hash == null ? newest.get(e.crate) !== e : !used.has(e.hash)),
  );
}

/** Bytes under `path` and its newest mtime, without following links. */
function measure(path) {
  const st = lstatSync(path);
  if (!st.isDirectory()) return { bytes: st.size, mtimeMs: st.mtimeMs };
  let bytes = 0;
  let mtimeMs = st.mtimeMs;
  for (const name of readdirSync(path)) {
    const inner = measure(join(path, name));
    bytes += inner.bytes;
    mtimeMs = Math.max(mtimeMs, inner.mtimeMs);
  }
  return { bytes, mtimeMs };
}

function list(dir) {
  try {
    return readdirSync(dir);
  } catch {
    return [];
  }
}

/** Every hashed copy under a `target/<profile>` directory. */
function entriesOf(profileDir) {
  const entries = [];
  for (const name of list(join(profileDir, "deps"))) {
    const hash = unitHash(name);
    if (hash) entries.push({ path: join(profileDir, "deps", name), hash });
  }
  for (const name of list(join(profileDir, "build"))) {
    const hash = unitHash(name);
    if (hash) entries.push({ path: join(profileDir, "build", name), hash });
  }
  for (const name of list(join(profileDir, "incremental"))) {
    // `<crate>-<stable crate id>`
    entries.push({ path: join(profileDir, "incremental", name), hash: null, crate: name.replace(/-[^-]+$/, "") });
  }
  return entries.map((e) => ({ ...e, ...measure(e.path) }));
}

/** Prune `profileDir` against the hashes the dev build used. Never throws. */
export function pruneTarget({ profileDir, used, log, now = Date.now() }) {
  try {
    const stale = planPrune(entriesOf(profileDir), used, now);
    if (!stale.length) return;
    let bytes = 0;
    for (const e of stale) {
      rmSync(e.path, { recursive: true, force: true });
      bytes += e.bytes;
    }
    log(`pruned ${(bytes / 1e9).toFixed(1)} GB of build output unused for ${PRUNE_DAYS} days`);
  } catch (e) {
    log(`could not prune old build output (${e.message}) — carrying on`);
  }
}
