// The release bundle is only coherent if three things agree: the sidecars the
// tauri configs list, the beforeBuildCommand that produces each, and CI, which
// must produce them before cargo needs them. A sidecar added to one and not the
// others is a bundle that builds locally and ships without it (keyd, once).
import { expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { cargoArgs } from "./keyd-build.mjs";
import { app } from "./runtime.mjs";

const rust = join(app, "src-tauri");
const json = (p) => JSON.parse(readFileSync(p, "utf8"));
const base = json(join(rust, "tauri.conf.json"));
const macos = json(join(rust, "tauri.macos.conf.json"));
const pkg = json(join(app, "package.json"));
const ci = readFileSync(join(app, "..", ".github", "workflows", "ci.yml"), "utf8");

const baseBins = base.bundle.externalBin;
const macosBins = macos.bundle.externalBin;
const before = base.build.beforeBuildCommand;

// The script in beforeBuildCommand that produces each sidecar.
const producers = {
  "binaries/ffmpeg": "bun run ffmpeg",
  "binaries/oculus": "bun run stage-cli",
  "binaries/apple-speech": "bun run speech",
  "binaries/whisper-cli": "bun run whisper",
  "binaries/oculus-keyd": "bun run stage-keyd",
};

test("macOS's externalBin repeats the base list and adds the macOS sidecars", () => {
  // Merge patches replace arrays, so the macOS file must list everything.
  expect(macosBins.slice(0, baseBins.length)).toEqual(baseBins);
  expect(macosBins.slice(baseBins.length).sort()).toEqual([
    "binaries/apple-speech",
    "binaries/oculus-keyd",
    "binaries/whisper-cli",
  ]);
});

test("keyd ships on macOS only, where it has an adapter", () => {
  expect(baseBins).not.toContain("binaries/oculus-keyd");
  expect(macosBins).toContain("binaries/oculus-keyd");
});

test("beforeBuildCommand produces every sidecar, keyd before the CLI", () => {
  for (const bin of macosBins) {
    expect(producers[bin], `no producer known for ${bin}`).toBeDefined();
    expect(before, bin).toContain(producers[bin]);
  }
  // tauri-build, run by the CLI's build, needs keyd's sidecar to exist.
  expect(before.indexOf("bun run stage-keyd")).toBeLessThan(before.indexOf("bun run stage-cli"));
});

test("every producer is a package script over a script that exists", () => {
  for (const command of Object.values(producers)) {
    const script = command.replace("bun run ", "");
    const run = pkg.scripts[script];
    expect(run, script).toBeDefined();
    expect(existsSync(join(app, run.replace(/^(node|bun) /, ""))), run).toBe(true);
  }
});

test("CI stages keyd before the first cargo build that needs its sidecar", () => {
  const keyd = ci.indexOf("bun run stage-keyd");
  const cli = ci.indexOf("bun run stage-cli");
  expect(keyd).toBeGreaterThan(-1);
  expect(keyd).toBeLessThan(cli);
});

test("the bundled keyd is built without the dev feature, and the dev one with it", () => {
  expect(cargoArgs("bundle", "x")).not.toContain("dev");
  expect(cargoArgs("bundle", "x")).not.toContain("--features");
  expect(cargoArgs("dev", "x").join(" ")).toContain("--features dev");
  for (const variant of ["dev", "bundle"]) expect(cargoArgs(variant, "x")).toContain("--locked");
});

test("the bundle is signed hardened, with library validation on", () => {
  const mac = base.bundle.macOS;
  expect(mac.signingIdentity).toBe("-");
  expect(mac.hardenedRuntime).toBe(true);
  // Tauri would put entitlements on keyd too, where they join its code hash.
  expect(mac.entitlements).toBeUndefined();
});
