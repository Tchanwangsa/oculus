import { afterEach, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdtempSync, mkdirSync, readFileSync, realpathSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const roots = [];
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });

function fixture() {
  const app = realpathSync(mkdtempSync(join(tmpdir(), "oculus-math build ")));
  roots.push(app);
  for (const dir of ["scripts", "math-core/katex", "math-core/edit", "math-core/wasm", "math-core/pkg", "node_modules/binaryen/bin"]) {
    mkdirSync(join(app, dir), { recursive: true });
  }
  copyFileSync(new URL("./build-math.mjs", import.meta.url), join(app, "scripts/build-math.mjs"));
  writeFileSync(join(app, "scripts/runtime.mjs"), `export const app = ${JSON.stringify(app)};`);
  writeFileSync(join(app, "math-core/Cargo.lock"), '[[package]]\r\nname = "wasm-bindgen"\r\nversion = "0.2.129"\r\n');
  writeFileSync(join(app, "node_modules/binaryen/bin/wasm-opt"), "fixture JavaScript entry");
  const wasm = join(app, "math-core/pkg/oculus_math_bg.wasm");
  writeFileSync(wasm, "last good wasm");
  const old = new Date(Date.now() - 60_000);
  utimesSync(wasm, old, old);
  const hook = join(app, "mock-tools.cjs");
  writeFileSync(hook, `
const assert = require("node:assert/strict");
const child = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const root = __dirname;
child.execFileSync = (command, args) => {
  if (command === "rustup") return "x86_64-pc-windows-msvc\\r\\nwasm32-unknown-unknown\\r\\n";
  if (command === "wasm-bindgen" && args[0] === "--version") return "wasm-bindgen " + (process.env.MATH_TEST_CLI_VERSION || "0.2.129") + "\\r\\n";
  if (command === "cargo") return "";
  if (command === "wasm-bindgen") {
    const out = args[args.indexOf("--out-dir") + 1];
    fs.mkdirSync(out, { recursive: true });
    for (const file of ["oculus_math.js", "oculus_math.d.ts", "oculus_math_bg.wasm.d.ts", "oculus_math_bg.wasm"]) {
      fs.writeFileSync(path.join(out, file), "generated " + file);
    }
    return "";
  }
  assert.equal(command, process.execPath, "wasm-opt must run through Node, without a shell or batch shim");
  assert.equal(args[0], path.join(root, "node_modules/binaryen/bin/wasm-opt"));
  assert.equal(args[1], "-Oz");
  fs.writeFileSync(args[args.indexOf("-o") + 1], "optimized wasm");
  return "";
};
require("node:module").syncBuiltinESMExports();
`);
  return { app, wasm, hook };
}

function build(input, env = {}) {
  return spawnSync("node", ["--require", input.hook, join(input.app, "scripts/build-math.mjs")], {
    encoding: "utf8", env: { ...process.env, ...env }, windowsHide: true,
  });
}

test("math build accepts Windows line endings and runs Binaryen's JS entry through Node", () => {
  const input = fixture();
  const result = build(input);
  expect(result.error).toBeUndefined();
  expect(result.status, result.stderr).toBe(0);
  expect(readFileSync(input.wasm, "utf8")).toBe("optimized wasm");
  expect(readFileSync(join(input.app, "math-core/pkg/oculus_math.js"), "utf8")).toBe("generated oculus_math.js");
});

test("math build rejects a mismatched CLI before replacing the previous wasm", () => {
  const input = fixture();
  const result = build(input, { MATH_TEST_CLI_VERSION: "0.2.128" });
  expect(result.status).toBe(1);
  expect(result.stderr).toContain("cargo install wasm-bindgen-cli --version 0.2.129 --locked");
  expect(readFileSync(input.wasm, "utf8")).toBe("last good wasm");
});
