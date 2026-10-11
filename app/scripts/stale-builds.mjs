// Ends the builds an earlier dev preflight of this checkout left behind, and
// names whoever holds cargo's build lock before a build waits on it. A stopped
// `tauri dev` does not take its preflight's synchronous cargo builds with it,
// so they keep compiling and holding the lock the next preflight needs.
//
// The pure half (parsing `ps`, choosing pids) is exported for
// stale-builds.test.mjs; the rest shells out to `ps` and `lsof` and is
// fail-open: any error logs one line and the preflight carries on. Windows has
// no `ps`/`lsof`, so every effect is skipped there.
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readlinkSync, realpathSync, unlinkSync, writeFileSync } from "node:fs";
import { basename, dirname, isAbsolute, join } from "node:path";

const unsupported = process.platform === "win32";

/** `ps -o pid=,ppid=,command=` output → `[{ pid, ppid, command }]`. */
export function parsePs(text) {
  const procs = [];
  for (const line of text.split("\n")) {
    const m = line.match(/^\s*(\d+)\s+(\d+)\s+(.*\S)\s*$/);
    if (m) procs.push({ pid: Number(m[1]), ppid: Number(m[2]), command: m[3] });
  }
  return procs;
}

/** The paths that identify this checkout's builds, for each spelling of `app`. */
export function checkoutPaths(apps) {
  const unique = [...new Set(apps)];
  return {
    apps: unique,
    manifests: unique.map((a) => join(a, "src-tauri", "Cargo.toml")),
    keydCrates: unique.map((a) => join(a, "keyd")),
    predevScripts: unique.map((a) => join(a, "scripts", "predev.mjs")),
    watchers: unique.map((a) => join(a, "scripts", "watch-cli.mjs")),
  };
}

const words = (command) => command.split(/\s+/);

/** The subcommand of a cargo command line (`build`, `check`, …), or null if it is not cargo. */
export function cargoSubcommand(command) {
  const [argv0, ...rest] = words(command);
  if (basename(argv0) !== "cargo") return null;
  return rest.find((w) => !w.startsWith("-") && !w.startsWith("+")) ?? null;
}

const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** `cargo build --manifest-path <ours> --bin oculus` (`runtime.mjs` `cliBuildArgs`). */
export function isOurCliBuild(command, paths) {
  if (cargoSubcommand(command) !== "build" || !/\s--bin[\s=]oculus(\s|$)/.test(command)) return false;
  return paths.manifests.some((m) => new RegExp(`\\s--manifest-path[\\s=]${escape(m)}(\\s|$)`).test(command));
}

/** keyd's `cargo build`, whose rustflags remap this checkout's crate to `/keyd` (`keyd-build.mjs`). */
export function isOurKeydBuild(command, paths) {
  if (cargoSubcommand(command) !== "build") return false;
  return paths.keydCrates.some((c) => command.includes(`--remap-path-prefix=${c}=/keyd`));
}

/** The script a `node`/`bun` command line runs, or null. */
function scriptOf(command) {
  const [argv0, ...rest] = words(command);
  if (!/^(node|bun)/.test(basename(argv0))) return null;
  return rest.find((w) => !w.startsWith("-")) ?? null;
}

/**
 * Whether `proc` is this checkout's predev. A relative `scripts/predev.mjs`
 * is checked against the process's cwd; `requireCwd` refuses one whose cwd is
 * unknown, which only the pidfile's own pid is spared.
 */
export function isPredevOf(proc, paths, cwd, requireCwd = true) {
  const script = scriptOf(proc.command);
  if (!script || basename(script) !== "predev.mjs") return false;
  if (isAbsolute(script)) return paths.predevScripts.includes(script);
  if (cwd == null) return !requireCwd;
  return paths.apps.some((a) => join(a, script) === join(a, "scripts", "predev.mjs") && cwd === a);
}

/** This checkout's CLI watcher, which predev starts by absolute path. */
export function isOurWatcher(command, paths) {
  const script = scriptOf(command);
  return script != null && paths.watchers.includes(script);
}

/** `pid` and every process above it. */
export function ancestorsOf(procs, pid) {
  const byPid = new Map(procs.map((p) => [p.pid, p]));
  const seen = new Set();
  for (let p = byPid.get(pid); p && !seen.has(p.pid); p = byPid.get(p.ppid)) seen.add(p.pid);
  seen.add(pid);
  return seen;
}

/** `roots` and everything below them, parents before children. */
export function treeOf(procs, roots) {
  const children = new Map();
  for (const p of procs) {
    if (!children.has(p.ppid)) children.set(p.ppid, []);
    children.get(p.ppid).push(p);
  }
  const order = [];
  const seen = new Set();
  const queue = [...roots];
  while (queue.length) {
    const pid = queue.shift();
    if (seen.has(pid)) continue;
    seen.add(pid);
    order.push(pid);
    for (const c of children.get(pid) ?? []) queue.push(c.pid);
  }
  return order;
}

/**
 * The processes to end: a previous predev of this checkout (the pidfile's, or
 * any found by cwd), this checkout's CLI watcher (predev rebuilds the CLI and
 * starts a new one, and a quick restart never trips its dev-server probe), and
 * orphaned (ppid 1) cargo builds of this checkout's CLI or keyd, each with its
 * whole tree. Our own ancestors and tree are never
 * picked. `cwdOf(pid)` returns a cwd or null.
 */
export function planCleanup({ procs, paths, selfPid, stalePid = null, cwdOf = () => null }) {
  const protectedPids = new Set([1, ...ancestorsOf(procs, selfPid), ...treeOf(procs, [selfPid])]);
  const roots = [];
  for (const p of procs) {
    if (protectedPids.has(p.pid)) continue;
    if (scriptOf(p.command) && basename(scriptOf(p.command)) === "predev.mjs") {
      if (isPredevOf(p, paths, cwdOf(p.pid), p.pid !== stalePid)) {
        roots.push({ pid: p.pid, what: "a previous preflight" });
      }
    } else if (isOurWatcher(p.command, paths)) {
      roots.push({ pid: p.pid, what: "the CLI watcher" });
    } else if (p.ppid === 1 && isOurCliBuild(p.command, paths)) {
      roots.push({ pid: p.pid, what: "an orphaned cargo build of oculus" });
    } else if (p.ppid === 1 && isOurKeydBuild(p.command, paths)) {
      roots.push({ pid: p.pid, what: "an orphaned cargo build of oculus-keyd" });
    }
  }
  const pids = treeOf(procs, roots.map((r) => r.pid)).filter((pid) => !protectedPids.has(pid));
  return { roots, pids };
}

/** A short name for the process holding a lock, from it and its ancestors. */
export function describeHolder(procs, pid) {
  const byPid = new Map(procs.map((p) => [p.pid, p]));
  const self = byPid.get(pid);
  if (!self) return `pid ${pid}`;
  const line = [...ancestorsOf(procs, pid)].map((a) => byPid.get(a)?.command ?? "");
  const sub = cargoSubcommand(self.command);
  const what = sub ? `cargo ${sub}` : basename(words(self.command)[0]);
  let owner = "";
  if (line.some((c) => /rust-analyzer/.test(c))) owner = "rust-analyzer's ";
  else if (line.some((c) => /watch-cli\.mjs/.test(c))) owner = "the CLI watcher's ";
  else if (line.some((c) => /predev\.mjs/.test(c))) owner = "another preflight's ";
  else if (self.ppid === 1) owner = "an orphaned ";
  return `${owner}${what} (pid ${pid})`;
}

/** The lock holders worth naming: everyone outside our own line of processes. */
export function foreignHolders(procs, holderPids, selfPid) {
  const ours = new Set([...ancestorsOf(procs, selfPid), ...treeOf(procs, [selfPid])]);
  return [...new Set(holderPids)].filter((pid) => !ours.has(pid));
}

function listProcesses() {
  return parsePs(execFileSync("ps", ["-Aww", "-o", "pid=,ppid=,command="], { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 }));
}

function cwdOf(pid) {
  try {
    if (process.platform === "linux") return readlinkSync(`/proc/${pid}/cwd`);
    const out = execFileSync("lsof", ["-a", "-p", String(pid), "-d", "cwd", "-Fn"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    return out.split("\n").find((l) => l.startsWith("n"))?.slice(1) ?? null;
  } catch {
    return null;
  }
}

const alive = (pid) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return e.code === "EPERM";
  }
};

const signal = (pid, sig) => {
  try {
    process.kill(pid, sig);
  } catch {
    // Already gone, or not ours to signal.
  }
};

const sleep = (ms) => Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);

/**
 * Stop the whole tree first so no cargo starts another rustc mid-way, pick up
 * any child spawned before its parent stopped, then SIGTERM, and SIGKILL what
 * is left after a short grace.
 */
function terminate(roots, pids) {
  let all = [...pids];
  for (const pid of all) signal(pid, "SIGSTOP");
  for (let pass = 0; pass < 3; pass++) {
    const more = treeOf(listProcesses(), roots).filter((pid) => !all.includes(pid));
    if (!more.length) break;
    for (const pid of more) signal(pid, "SIGSTOP");
    all = [...all, ...more];
  }
  for (const pid of all) signal(pid, "SIGTERM");
  for (const pid of all) signal(pid, "SIGCONT");
  const deadline = Date.now() + 2000;
  while (Date.now() < deadline && all.some(alive)) sleep(100);
  for (const pid of all.filter(alive)) signal(pid, "SIGKILL");
}

function spellings(app) {
  try {
    return [app, realpathSync(app)];
  } catch {
    return [app];
  }
}

/**
 * End the previous preflight's builds, then claim `pidFile` for this one until
 * it exits. Never throws.
 */
export function endStaleBuilds({ app, pidFile, log }) {
  if (unsupported) return;
  try {
    let stalePid = null;
    try {
      stalePid = Number(readFileSync(pidFile, "utf8").trim()) || null;
    } catch {
      // No previous preflight recorded.
    }
    const procs = listProcesses();
    const { roots, pids } = planCleanup({
      procs,
      paths: checkoutPaths(spellings(app)),
      selfPid: process.pid,
      stalePid,
      cwdOf,
    });
    if (roots.length) {
      log(`ending ${roots.map((r) => `${r.what} (pid ${r.pid})`).join(", ")} — ${pids.length} processes`);
      terminate(roots.map((r) => r.pid), pids);
    }
  } catch (e) {
    log(`could not check for a previous preflight's builds (${e.message}) — carrying on`);
  }
  try {
    mkdirSync(dirname(pidFile), { recursive: true });
    writeFileSync(pidFile, String(process.pid));
    process.on("exit", () => {
      try {
        if (readFileSync(pidFile, "utf8").trim() === String(process.pid)) unlinkSync(pidFile);
      } catch {
        // Already gone.
      }
    });
  } catch (e) {
    log(`could not record this preflight in ${pidFile} (${e.message}) — carrying on`);
  }
}

/**
 * Log who holds cargo's locks in `profileDir` (a `target/<profile>` directory)
 * when it is not us, so cargo's "Blocking waiting for file lock" has a name.
 * `lock` names the lock in that line ("the build lock").
 */
export function nameLockHolder(profileDir, lock, log) {
  if (unsupported) return;
  try {
    const locks = [".cargo-lock", ".cargo-build-lock", ".cargo-artifact-lock"]
      .map((f) => join(profileDir, f))
      .filter((f) => existsSync(f));
    if (!locks.length) return;
    let out = "";
    try {
      out = execFileSync("lsof", ["-t", ...locks], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] });
    } catch (e) {
      // lsof exits 1 when no process has any of the files open.
      if (e.status !== 1) throw e;
      out = e.stdout ?? "";
    }
    const holders = out.split("\n").map(Number).filter(Boolean);
    if (!holders.length) return;
    const procs = listProcesses();
    const names = foreignHolders(procs, holders, process.pid).map((pid) => describeHolder(procs, pid));
    if (names.length) {
      log(`${names.join(" and ")} ${names.length > 1 ? "hold" : "holds"} ${lock}; waiting for it`);
    }
  } catch (e) {
    log(`could not tell who holds the build lock (${e.message}) — carrying on`);
  }
}
