// Rebuild the debug CLI after Rust/template changes until the dev server ends.
// The debounce lets Tauri's app rebuild take cargo's lock first.
import { execFile } from "node:child_process";
import { readFileSync, rmSync, unlinkSync, watch, writeFileSync } from "node:fs";
import { connect } from "node:net";
import { join } from "node:path";
import { app, cliBuildArgs, cliPath, rust, stageCli } from "./runtime.mjs";

const cli = cliPath("debug");
const pidFile = join(rust, "target", ".oculus-cli-watch.pid");

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

// One watcher per checkout: a previous session that outlived its dev server
// would otherwise keep compiling against the same target directory.
try {
  const prev = Number(readFileSync(pidFile, "utf8").trim());
  if (prev && prev !== process.pid) process.kill(prev, "SIGTERM");
} catch {
  // No pid file, or the process is already gone.
}
writeFileSync(pidFile, String(process.pid));

function bye(reason) {
  log(reason);
  try {
    unlinkSync(pidFile);
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
  // Same reason as predev: a build that leaves no binary must not pass on an
  // old one.
  rmSync(cli, { force: true });
  execFile(
    "cargo",
    cliBuildArgs("debug"),
    { maxBuffer: 32 * 1024 * 1024 },
    (err, _out, stderr) => {
      building = false;
      if (err) {
        // Not fatal — the same error is in front of them in tauri's own output,
        // and the session should keep running while they fix it.
        log("build failed:");
        process.stderr.write(stderr);
      } else {
        // As in predev: the next app build copies the sidecar over this one.
        try {
          stageCli(cli);
          log(`oculus rebuilt in ${((Date.now() - started) / 1000).toFixed(1)}s`);
        } catch (e) {
          log(`oculus rebuilt but not staged as the sidecar: ${e.message}`);
        }
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
  // `localhost`, not 127.0.0.1: vite binds whichever family localhost resolves
  // to, which on macOS is ::1.
  const sock = connect({ port: devPort, host: "localhost" });
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
