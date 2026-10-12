import { expect, test } from "bun:test";
import {
  checkoutPaths,
  describeHolder,
  foreignHolders,
  isOurCliBuild,
  isOurKeydBuild,
  parsePs,
  planCleanup,
} from "./stale-builds.mjs";

const app = "/u/gh/oculus/app";
const tree = "/u/gh/oculus/.claude/worktrees/w1/app";
const paths = checkoutPaths([app]);
const tc = "/u/.rustup/toolchains/stable-aarch64-apple-darwin/bin";
const cliBuild = (a) => `${tc}/cargo build --manifest-path ${a}/src-tauri/Cargo.toml --bin oculus`;
const keydBuild = (a) =>
  `${tc}/cargo build --release --locked --features dev --config build.rustflags=["--remap-path-prefix=${a}/keyd=/keyd", "--remap-path-prefix=/u/.cargo=/cargo"]`;
const rustc = (name) => `${tc}/rustc --crate-name ${name} --edition=2021 src/lib.rs`;

// Our own chain: tauri dev → bun run dev → node scripts/predev.mjs (pid 500).
const ours = `
    1     0 /sbin/launchd
  400   300 bun run tauri dev
  450   400 /bin/sh -c OCULUS_CLI_WATCH=1 bun run dev
  480   450 bun run dev
  500   480 node scripts/predev.mjs
`;
const self = 500;
const cwds = { 500: app };
const cwdOf = (pid) => cwds[pid] ?? null;
const plan = (extra, opts = {}) =>
  planCleanup({ procs: parsePs(ours + extra), paths, selfPid: self, cwdOf, ...opts });

test("parses ps output with spaces in the command", () => {
  const procs = parsePs("  46164 45882 /Users/u/Library/Application Support/Zed/rust-analyzer\n\n  7 1 x \n");
  expect(procs).toEqual([
    { pid: 46164, ppid: 45882, command: "/Users/u/Library/Application Support/Zed/rust-analyzer" },
    { pid: 7, ppid: 1, command: "x" },
  ]);
});

test("an orphaned CLI build of this checkout is picked with its rustc children", () => {
  const { roots, pids } = plan(`
  900     1 ${cliBuild(app)}
  901   900 ${rustc("app_lib")}
  902   900 sccache ${rustc("keyd_core")}
  903   902 ${rustc("keyd_core")}
  `);
  expect(roots.map((r) => r.pid)).toEqual([900]);
  expect(pids).toEqual([900, 901, 902, 903]);
});

test("an orphaned keyd build of this checkout is picked", () => {
  const { roots, pids } = plan(`
  910     1 ${keydBuild(app)}
  911   910 ${rustc("oculus_keyd")}
  `);
  expect(roots.map((r) => r.pid)).toEqual([910]);
  expect(pids).toEqual([910, 911]);
});

test("a worktree's builds are left alone", () => {
  const { roots, pids } = plan(`
  920     1 ${cliBuild(tree)}
  921   920 ${rustc("app_lib")}
  922     1 ${keydBuild(tree)}
  930   929 node scripts/predev.mjs
  `);
  cwds[930] = tree;
  expect(roots).toEqual([]);
  expect(pids).toEqual([]);
  delete cwds[930];
});

test("a path that merely ends with this checkout's is not this checkout", () => {
  expect(isOurCliBuild(cliBuild(`/x${app}`), paths)).toBe(false);
  expect(isOurKeydBuild(keydBuild(`/x${app}`), paths)).toBe(false);
  expect(isOurCliBuild(cliBuild(app), paths)).toBe(true);
});

test("rust-analyzer's cargo check is never picked, even orphaned", () => {
  const { roots } = plan(`
  600   590 /Users/u/Library/Application Support/Zed/languages/rust-analyzer/rust-analyzer-2026-10-05
  601   600 ${tc}/cargo check --workspace --message-format=json --manifest-path ${app}/src-tauri/Cargo.toml --all-targets
  602     1 ${tc}/cargo check --manifest-path ${app}/src-tauri/Cargo.toml --bin oculus
  `);
  expect(roots).toEqual([]);
});

test("a build started from a live shell is not an orphan", () => {
  const { roots } = plan(`
  700   690 -zsh
  701   700 ${cliBuild(app)}
  `);
  expect(roots).toEqual([]);
});

test("our own ancestors and tree are never picked", () => {
  const { roots, pids } = plan(`
  510   500 ${cliBuild(app)}
  511   510 ${rustc("app_lib")}
  `, { stalePid: 500 });
  expect(roots).toEqual([]);
  expect(pids).toEqual([]);
});

test("the pidfile's stale predev is ended with its whole tree, watcher included", () => {
  const { roots, pids } = plan(`
  200   180 bun run dev
  210   200 node scripts/predev.mjs
  211   210 ${cliBuild(app)}
  212   211 ${rustc("app_lib")}
  213   211 /u/gh/oculus/app/src-tauri/target/debug/build/libsqlite3-sys-1/build-script-build
  214   213 cc -c sqlite3.c
  215   210 node /u/gh/oculus/app/scripts/build-math.mjs
  216   210 node /u/gh/oculus/app/scripts/watch-cli.mjs
  217   216 ${cliBuild(app)}
  `, { stalePid: 210 });
  expect(roots.map((r) => r.pid)).toEqual([210, 216]);
  expect(pids.sort()).toEqual([210, 211, 212, 213, 214, 215, 216, 217]);
});

test("this checkout's detached watcher is ended with its build; another checkout's is not", () => {
  const { roots, pids } = plan(`
  600     1 node ${app}/scripts/watch-cli.mjs
  601   600 ${cliBuild(app)}
  602   601 ${rustc("app_lib")}
  610     1 node ${tree}/scripts/watch-cli.mjs
  611   610 ${cliBuild(tree)}
  `);
  expect(roots).toEqual([{ pid: 600, what: "the CLI watcher" }]);
  expect(pids).toEqual([600, 601, 602]);
});

test("an orphaned dev server of this checkout is ended with its parent; a live or foreign one is not", () => {
  const vite = (a) => `node ${a}/node_modules/.bin/vite`;
  const orphan = plan(`
  700     1 bun run dev
  701   700 ${vite(app)}
  710     1 ${vite(app)}
  720     1 bun run dev
  721   720 ${vite(tree)}
  730   300 bun run tauri dev
  731   730 bun run dev
  732   731 ${vite(app)}
`);
  expect(orphan.roots).toEqual([
    { pid: 700, what: "an orphaned dev server" },
    { pid: 710, what: "an orphaned dev server" },
  ]);
  expect(orphan.pids).toEqual([700, 710, 701]);
});

test("the pidfile's predev is trusted without a cwd; others need theirs", () => {
  const extra = `
  210   200 node scripts/predev.mjs
  220   200 node scripts/predev.mjs
  `;
  expect(plan(extra, { stalePid: 210 }).roots.map((r) => r.pid)).toEqual([210]);
  cwds[220] = app;
  expect(plan(extra, { stalePid: 210 }).roots.map((r) => r.pid)).toEqual([210, 220]);
  delete cwds[220];
});

test("a reused pidfile pid running something else is not killed", () => {
  const { roots, pids } = plan(`
  210   200 /Applications/Safari.app/Contents/MacOS/Safari
  211   210 node scripts/predev.mjs-notes
  `, { stalePid: 210 });
  expect(roots).toEqual([]);
  expect(pids).toEqual([]);
});

test("a reused pidfile pid running another checkout's predev is not killed", () => {
  cwds[210] = tree;
  const { roots } = plan(`
  210   200 node scripts/predev.mjs
  `, { stalePid: 210 });
  expect(roots).toEqual([]);
  delete cwds[210];
});

test("lock holders are named by who runs them, and our own tree is not named", () => {
  const procs = parsePs(ours + `
  600   590 /Users/u/Library/Application Support/Zed/languages/rust-analyzer/rust-analyzer-2026-10-05
  601   600 ${tc}/cargo check --workspace --manifest-path ${app}/src-tauri/Cargo.toml
  700     1 ${cliBuild(app)}
  510   500 ${cliBuild(app)}
  `);
  expect(foreignHolders(procs, [601, 510, 601], self)).toEqual([601]);
  expect(describeHolder(procs, 601)).toBe("rust-analyzer's cargo check (pid 601)");
  expect(describeHolder(procs, 700)).toBe("an orphaned cargo build (pid 700)");
});
