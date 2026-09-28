#!/usr/bin/env node
// Keeps `target/debug/oculus` in step with the Rust sources for as long as a
// dev session lasts.
//
// `scripts/predev.mjs` builds the CLI once, before the app starts. That is not
// enough on its own: `tauri dev` re-runs cargo and relaunches the app on every
// Rust change, but it never re-runs `beforeDevCommand`, so on a long session
// the CLI drifts back to the vintage of whenever the session began — which is
// the same silent staleness, just measured in hours instead of days. The app's
// own binary is rebuilt for free by the watcher tauri already runs; this is
// that watcher's missing half.
//
// It deliberately loses the race with tauri's rebuild. The debounce here is
// longer than tauri's, so the app's cargo build takes the package lock first
// and the CLI build waits behind it — the dev feedback loop keeps the latency
// it had, and the CLI catches up in the background a few seconds after the app
// has already relaunched.
//
// Started detached by `predev --watch`, so nothing kills it when the hook's
// shell exits. It ends itself by watching the vite dev server: when the port
// in tauri.conf.json stops answering, the session is over.

import { execFile } from "node:child_process";
import { closeSync, openSync, readFileSync, unlinkSync, watch, writeFileSync } from "node:fs";
import { connect } from "node:net";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const app = dirname(dirname(fileURLToPath(import.meta.url)));
const manifest = join(app, "src-tauri", "Cargo.toml");
const target = join(app, "src-tauri", "target");
const exe = process.platform === "win32" ? ".exe" : "";
const cli = join(target, "debug", `oculus${exe}`);
const pidFile = join(target, ".oculus-cli-watch.pid");

/** Long enough that tauri's own rebuild reaches the package lock first. */
const DEBOUNCE_MS = 5000;
/** How long the dev server may take to come up before we give up on it. */
const STARTUP_GRACE_MS = 180_000;
const PROBE_EVERY_MS = 20_000;

const log = (msg) => console.log(`[cli-watch] ${msg}`);

// Detached, with the dev terminal's stdout inherited: if that terminal is
// closed while the watcher is still up, a log line would otherwise take the
// process down with EPIPE.
process.stdout.on("error", () => {});
process.stderr.on("error", () => {});

const devPort = (() => {
  try {
    const conf = JSON.parse(readFileSync(join(app, "src-tauri", "tauri.conf.json"), "utf8"));
    return Number(new URL(conf.build.devUrl).port) || 1420;
  } catch {
    return 1420;
  }
})();

// One watcher per checkout. Never kill a process based only on a saved PID:
// after a crash Windows can reuse that number for an unrelated application.
let lockOwned = false;
for (let attempt = 0; attempt < 2 && !lockOwned; attempt++) {
  try {
    const fd = openSync(pidFile, "wx");
    try { writeFileSync(fd, String(process.pid)); } finally { closeSync(fd); }
    lockOwned = true;
  } catch (error) {
    if (error.code !== "EEXIST") throw error;
    let previous = 0;
    try { previous = Number(readFileSync(pidFile, "utf8").trim()); } catch {}
    if (previous > 0) {
      try {
        process.kill(previous, 0);
        log("a watcher is already active for this checkout");
        process.exit(0);
      } catch (probeError) {
        if (probeError.code !== "ESRCH") {
          log("cannot verify the existing watcher; leaving it alone");
          process.exit(0);
        }
      }
    } else {
      // Another preflight may still be writing the lock it just acquired.
      log("another watcher is starting for this checkout");
      process.exit(0);
    }
    try { unlinkSync(pidFile); } catch {}
  }
}
if (!lockOwned) process.exit(0);

function bye(reason) {
  log(reason);
  try {
    if (readFileSync(pidFile, "utf8").trim() === String(process.pid)) unlinkSync(pidFile);
  } catch {
    // Already replaced by a newer watcher.
  }
  process.exit(0);
}
for (const sig of ["SIGTERM", "SIGINT", "SIGHUP"]) {
  process.on(sig, () => bye("stopping"));
}

let timer = null;
let building = false;
let again = false;

function build() {
  if (building) {
    again = true;
    return;
  }
  building = true;
  const started = Date.now();
  // stage-cli owns stale-binary removal. Run it in the child so a running
  // Windows CLI holding its .exe open cannot crash this watcher permanently.
  execFile(
    process.execPath,
    [join(app, "scripts", "stage-cli.mjs"), "--debug"],
    { maxBuffer: 32 * 1024 * 1024, windowsHide: true },
    (err, _out, stderr) => {
      building = false;
      if (err) {
        // Not fatal — the same error is in front of them in tauri's own output,
        // and the session should keep running while they fix it.
        log("build failed:");
        process.stderr.write(stderr);
        if (process.platform === "win32" && /EPERM|EBUSY|EACCES/.test(stderr)
            && /oculus\.exe/i.test(stderr)) {
          log("the CLI is still in use; retrying after it releases the executable");
          clearTimeout(timer);
          timer = setTimeout(build, DEBOUNCE_MS);
        }
      } else {
        log(`oculus rebuilt in ${((Date.now() - started) / 1000).toFixed(1)}s`);
      }
      if (again) {
        again = false;
        build();
      }
    },
  );
}

function schedule(file) {
  if (!/\.(rs|toml|lock|md|json)$/.test(file ?? "")) return;
  clearTimeout(timer);
  timer = setTimeout(build, DEBOUNCE_MS);
}

// `templates/` is watched with `src/`: the agent-facing docs are `include_str!`
// into the same library the CLI links, so a template edit changes what
// `oculus docs` writes.
for (const dir of ["src", "templates"]) {
  try {
    watch(join(app, "src-tauri", dir), { recursive: true }, (_e, file) => schedule(file));
  } catch (e) {
    log(`cannot watch ${dir}: ${e.message}`);
  }
}
for (const file of ["Cargo.toml", "Cargo.lock"]) {
  try {
    watch(join(app, "src-tauri", file), () => schedule(file));
  } catch {
    // Cargo.lock may not exist yet on a cold checkout.
  }
}

const startedAt = Date.now();
let sawServer = false;
let misses = 0;

function probe() {
  const sock = connect({ port: devPort, host: "127.0.0.1" });
  let settled = false;
  const done = (up) => {
    // A timeout is routinely followed by an error on the same socket, and a
    // miss counted twice would end the session a probe early.
    if (settled) return;
    settled = true;
    sock.destroy();
    if (up) {
      sawServer = true;
      misses = 0;
      return;
    }
    if (sawServer && ++misses >= 3) bye("dev server is gone");
    if (!sawServer && Date.now() - startedAt > STARTUP_GRACE_MS) {
      bye("no dev server appeared");
    }
  };
  sock.setTimeout(2000);
  sock.once("connect", () => done(true));
  sock.once("timeout", () => done(false));
  sock.once("error", () => done(false));
}

setInterval(probe, PROBE_EVERY_MS);
log(`watching src-tauri/src for changes (dev server on :${devPort})`);
