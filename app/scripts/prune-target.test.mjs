import { expect, test } from "bun:test";
import { planPrune, PRUNE_DAYS, unitHash, usedHashes } from "./prune-target.mjs";

const A = "0123456789abcdef";
const B = "fedcba9876543210";
const DAY = 24 * 60 * 60 * 1000;
const now = 100 * DAY;
const old = now - (PRUNE_DAYS + 1) * DAY;
const recent = now - DAY;

test("a unit's hash is read from its deps files and build directories", () => {
  expect(unitHash(`libserde-${A}.rlib`)).toBe(A);
  expect(unitHash(`libserde-${A}.rmeta`)).toBe(A);
  expect(unitHash(`app_lib-${A}.app_lib.1f2e3d-cgu.0.rcgu.o`)).toBe(A);
  expect(unitHash(`oculus-${A}`)).toBe(A);
  expect(unitHash(`serde-${A}`)).toBe(A);
  expect(unitHash("libapp_lib.rlib")).toBe(null);
  expect(unitHash("app_lib-2rktjyygf3y4m")).toBe(null);
});

test("the used hashes come from every artifact and build script cargo reports", () => {
  const stdout = [
    JSON.stringify({
      reason: "compiler-artifact",
      fresh: true,
      filenames: [`/t/debug/deps/libserde-${A}.rlib`, `/t/debug/deps/libserde-${A}.rmeta`],
      executable: null,
    }),
    JSON.stringify({
      reason: "compiler-artifact",
      filenames: [`/t/debug/build/app-${B}/build-script-build`],
    }),
    JSON.stringify({ reason: "build-script-executed", out_dir: "/t/debug/build/app-1111111111111111/out" }),
    JSON.stringify({ reason: "build-finished", success: true }),
    "not json",
  ].join("\n");
  expect([...usedHashes(stdout)].sort()).toEqual(["1111111111111111", A, B].sort());
});

test("only old copies the dev build did not use are pruned", () => {
  const entries = [
    { path: "used-old", hash: A, mtimeMs: old },
    { path: "unused-old", hash: B, mtimeMs: old },
    { path: "unused-recent", hash: "2222222222222222", mtimeMs: recent },
  ];
  expect(planPrune(entries, new Set([A]), now).map((e) => e.path)).toEqual(["unused-old"]);
});

test("the newest incremental cache of each crate is kept however old", () => {
  const entries = [
    { path: "app_lib-a", hash: null, crate: "app_lib", mtimeMs: old - DAY },
    { path: "app_lib-b", hash: null, crate: "app_lib", mtimeMs: old },
    { path: "app_lib-c", hash: null, crate: "app_lib", mtimeMs: old - 2 * DAY },
    { path: "keyd_core-a", hash: null, crate: "keyd_core", mtimeMs: old },
    { path: "oculus-a", hash: null, crate: "oculus", mtimeMs: recent },
    { path: "oculus-b", hash: null, crate: "oculus", mtimeMs: recent - DAY },
  ];
  expect(planPrune(entries, new Set(), now).map((e) => e.path)).toEqual(["app_lib-a", "app_lib-c"]);
});
