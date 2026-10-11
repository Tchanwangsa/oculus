// Building, signing and installing oculus-keyd, shared by `build-keyd.mjs` (the
// dev build), `stage-keyd.mjs` (the bundle's) and the dev preflight. The build
// step for this OS is the only OS-specific part (docs/development.md); an OS
// without one builds nothing.
import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, existsSync, mkdirSync, realpathSync, renameSync, rmSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { app, buildCli, cliPath, helperProgram, keydHelperApp } from "./runtime.mjs";

export const log = (msg) => console.log(`[keyd] ${msg}`);

export const crate = realpathSync(join(app, "keyd"));

/** keyd's signing identifier: the helper's bundle identifier and launchd's label. */
export const KEYD_IDENTIFIER = "com.tchan.oculus.keyd";

// The helper app's Info.plist and icon. keyd's build.rs hashes both into its
// source hash, since both are sealed into the signature.
const helperPlist = join(crate, "bundle", "Info.plist");
const helperIcon = join(app, "src-tauri", "icons", "icon.icns");

/**
 * The two builds differ only in the `dev` feature, which admits any same-user
 * caller. A bundled release must not have it (keyd/Cargo.toml), and each signs
 * into its own directory so neither replaces the other's output.
 */
export const variants = {
  dev: { features: ["dev"], signedDir: join(crate, "target", "signed") },
  bundle: { features: [], signedDir: join(crate, "target", "bundle") },
};

/** `cargo build` arguments for a variant; `rustflags` is the `--config` value. */
export function cargoArgs(variant, rustflags) {
  const { features } = variants[variant];
  return [
    "build", "--release", "--locked",
    ...(features.length ? ["--features", features.join(",")] : []),
    "--config", rustflags,
  ];
}

// macOS: a reproducible build, so the same source gives the same bytes from
// any checkout and the signed cdhash the keychain trusts only changes when
// keyd's source does; then an ad-hoc signature with the hardened runtime.
function buildForMacos(variant, signedDir) {
  const home = homedir();
  const cargoHome = process.env.CARGO_HOME ?? join(home, ".cargo");
  const rustupHome = process.env.RUSTUP_HOME ?? join(home, ".rustup");

  // Path-dependent half of the reproducible flags; keyd/.cargo/config.toml has
  // the rest. Cargo joins a --config array onto the config file's, and reads
  // that file only when run from the crate, hence the cwd.
  const remaps = [[crate, "/keyd"], [cargoHome, "/cargo"], [rustupHome, "/rustup"]]
    .filter(([from]) => existsSync(from))
    .map(([from, to]) => `--remap-path-prefix=${realpathSync(from)}=${to}`);
  const rustflags = `build.rustflags=[${remaps.map((f) => JSON.stringify(f)).join(", ")}]`;

  const built = join(crate, "target", "release", "oculus-keyd");
  rmSync(built, { force: true });
  execFileSync("cargo", cargoArgs(variant, rustflags), { cwd: crate, stdio: "inherit" });
  if (!existsSync(built)) throw new Error(`cargo reported success but ${built} is not there`);

  // keyd runs as a helper app so macOS names it "Oculus Helper", with the
  // app's icon. Lay out and sign a fresh copy, then rename it into place:
  // re-signing a file that has already run leaves the kernel's cached
  // signature stale. Signing the bundle binds its Info.plist and seals the
  // icon; `-i` keeps the cdhash independent of the directory's name, and every
  // input is a checked-in or reproducibly built file, so the same source
  // gives the same cdhash.
  const signed = join(signedDir, keydHelperApp);
  const partial = join(signedDir, `.${keydHelperApp}.${process.pid}.part`);
  mkdirSync(signedDir, { recursive: true });
  rmSync(partial, { recursive: true, force: true });
  try {
    mkdirSync(join(partial, "Contents", "MacOS"), { recursive: true });
    mkdirSync(join(partial, "Contents", "Resources"), { recursive: true });
    copyFileSync(helperPlist, join(partial, "Contents", "Info.plist"));
    copyFileSync(helperIcon, join(partial, "Contents", "Resources", "icon.icns"));
    copyFileSync(built, helperProgram(partial));
    chmodSync(helperProgram(partial), 0o755);
    // codesign refuses extended attributes (Finder info, provenance) as detritus.
    execFileSync("xattr", ["-cr", partial]);
    execFileSync("codesign", ["-s", "-", "-f", "-o", "runtime", "-i", KEYD_IDENTIFIER, partial], { stdio: "inherit" });
    rmSync(signed, { recursive: true, force: true });
    renameSync(partial, signed);
  } finally {
    rmSync(partial, { recursive: true, force: true });
  }
  return signed;
}

// Each adapter's build step, by `process.platform`.
const buildSteps = { darwin: buildForMacos };

export const hasAdapter = () => process.platform in buildSteps;

/**
 * Build and sign keyd for `variant` ("dev" or "bundle"), returning the signed
 * helper app, the keyd inside it and its source hash, or null on an OS with no
 * adapter. `signedDir` overrides where the signed helper lands.
 */
export function buildKeyd(variant, signedDir = variants[variant].signedDir) {
  const step = buildSteps[process.platform];
  if (!step) {
    log(`oculus-keyd has no adapter for ${process.platform} — nothing built`);
    return null;
  }
  const helper = step(variant, signedDir);
  const binary = helperProgram(helper);
  const hash = execFileSync(binary, ["source-hash"], { encoding: "utf8" }).trim();
  log(`built ${helper} (${variant}, source ${hash.slice(0, 12)})`);
  return { helper, binary, hash };
}

/**
 * Install the helper app `helper` when the checkout is the main one and its
 * source changed. A worktree's build must never take over the registration
 * the running app depends on.
 */
export function installFromMainCheckout(helper) {
  const git = (...args) =>
    execFileSync("git", ["rev-parse", "--path-format=absolute", ...args], {
      cwd: app,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
  let main = false;
  try {
    main = realpathSync(git("--git-dir")) === realpathSync(git("--git-common-dir"));
  } catch {
    // No git, or not a checkout: nothing to install from.
  }
  if (!main) {
    log(`not the main checkout — not installed (by hand: oculus keyd install --from "${helper}")`);
    return;
  }

  // The CLI compares the installed stamp and registration with this build, so
  // an unchanged source installs nothing.
  const cli = process.env.OCULUS_BIN ?? cliPath("debug");
  if (!existsSync(cli)) buildCli("debug");
  execFileSync(cli, ["keyd", "install", "--if-changed", "--from", helper], { stdio: "inherit" });
}
