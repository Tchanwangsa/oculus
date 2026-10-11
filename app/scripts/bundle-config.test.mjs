// The release bundle is only coherent if three things agree: the sidecars and
// keyd's helper app the tauri configs list, the beforeBuildCommand that
// produces each, and CI, which must produce them before cargo needs them. A
// sidecar added to one and not the others is a bundle that builds locally and
// ships without it (keyd, once).
import { expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { KEYD_IDENTIFIER, cargoArgs } from "./keyd-build.mjs";
import { app, keydHelperApp } from "./runtime.mjs";

const rust = join(app, "src-tauri");
const json = (p) => JSON.parse(readFileSync(p, "utf8"));
const base = json(join(rust, "tauri.conf.json"));
const macos = json(join(rust, "tauri.macos.conf.json"));
const pkg = json(join(app, "package.json"));
const ci = readFileSync(join(app, "..", ".github", "workflows", "ci.yml"), "utf8");

const baseBins = base.bundle.externalBin;
const macosBins = macos.bundle.externalBin;
const macosFiles = macos.bundle.macOS?.files ?? {};
const before = base.build.beforeBuildCommand;
const paths = readFileSync(join(app, "keyd", "core", "src", "paths.rs"), "utf8");

// The script in beforeBuildCommand that produces each sidecar.
const producers = {
  "binaries/ffmpeg": "bun run ffmpeg",
  "binaries/oculus": "bun run stage-cli",
  "binaries/apple-speech": "bun run speech",
  "binaries/whisper-cli": "bun run whisper",
  [`binaries/${keydHelperApp}`]: "bun run stage-keyd",
};

test("macOS's externalBin repeats the base list and adds the macOS sidecars", () => {
  // Merge patches replace arrays, so the macOS file must list everything.
  expect(macosBins.slice(0, baseBins.length)).toEqual(baseBins);
  expect(macosBins.slice(baseBins.length).sort()).toEqual([
    "binaries/apple-speech",
    "binaries/whisper-cli",
  ]);
});

test("keyd ships on macOS only, as a helper app nested in the bundle", () => {
  // A directory, so `bundle.macOS.files`, which Tauri copies without
  // re-signing; an externalBin is a single file Tauri re-signs.
  for (const bin of [...baseBins, ...macosBins]) expect(bin).not.toContain("keyd");
  expect(base.bundle.macOS.files).toBeUndefined();
  expect(macosFiles).toEqual({ [`Helpers/${keydHelperApp}`]: `binaries/${keydHelperApp}` });
  // Merge patches merge objects, so the base signing settings still apply.
  expect(macos.bundle.macOS.signingIdentity).toBeUndefined();
});

test("the helper's name and identifier match keyd's own", () => {
  const helper = paths.match(/pub const HELPER: &str = "([^"]+)";/)?.[1];
  expect(`${helper}.app`).toBe(keydHelperApp);
  const plist = readFileSync(join(app, "keyd", "bundle", "Info.plist"), "utf8");
  expect(plist).toContain(`<key>CFBundleIdentifier</key>\n    <string>${KEYD_IDENTIFIER}</string>`);
  expect(plist).toContain(`<key>CFBundleName</key>\n    <string>${helper}</string>`);
});

test("beforeBuildCommand produces every sidecar and keyd's helper", () => {
  for (const bin of [...macosBins, ...Object.values(macosFiles)]) {
    expect(producers[bin], `no producer known for ${bin}`).toBeDefined();
    expect(before, bin).toContain(producers[bin]);
  }
});

test("every producer is a package script over a script that exists", () => {
  for (const command of Object.values(producers)) {
    const script = command.replace("bun run ", "");
    const run = pkg.scripts[script];
    expect(run, script).toBeDefined();
    expect(existsSync(join(app, run.replace(/^(node|bun) /, ""))), run).toBe(true);
  }
});

test("CI builds and stages keyd's helper as the bundle would carry it", () => {
  expect(ci).toContain("bun run stage-keyd");
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
  // Tauri would put entitlements on every sidecar; library validation needs
  // none, and keyd's helper is signed by its own build.
  expect(mac.entitlements).toBeUndefined();
});
