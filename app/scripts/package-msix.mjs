import { appendFileSync, closeSync, copyFileSync, cpSync, existsSync, mkdirSync, openSync, readFileSync, readSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const app = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const rust = join(app, "src-tauri");
const distribution = join(rust, "ffmpeg-notices");
const required = {
  IDENTITY_NAME: "OCULUS_MSIX_IDENTITY_NAME",
  PUBLISHER: "OCULUS_MSIX_PUBLISHER",
  DISPLAY_NAME: "OCULUS_MSIX_DISPLAY_NAME",
  PUBLISHER_DISPLAY_NAME: "OCULUS_MSIX_PUBLISHER_DISPLAY_NAME",
  VERSION: "OCULUS_MSIX_VERSION",
};

export function readMsixIdentity(env) {
  const missing = Object.values(required).filter((key) => !env[key]?.trim());
  if (missing.length) return { identity: null, missing };
  const identity = Object.fromEntries(Object.entries(required).map(([key, variable]) => [key, env[variable].trim()]));
  if (!/^[A-Za-z0-9.-]{3,50}$/.test(identity.IDENTITY_NAME)) throw new Error("Invalid Partner Center package identity name");
  if (!identity.PUBLISHER.startsWith("CN=")) throw new Error("Publisher must be the exact Partner Center distinguished name beginning CN=");
  const numbers = identity.VERSION.split(".");
  if (numbers.length !== 4 || numbers.some((part) => !/^(0|[1-9]\d*)$/.test(part) || Number(part) > 65535)
      || Number(numbers[0]) === 0 || numbers[3] !== "0") {
    throw new Error("OCULUS_MSIX_VERSION must have four integer parts (0–65535), a nonzero major and a final 0 reserved for the Store");
  }
  for (const value of Object.values(identity)) {
    if (/[\u0000-\u0008\u000b\u000c\u000e-\u001f]/.test(value)) throw new Error("Invalid control character in Store identity");
  }
  return { identity, missing: [] };
}

export function renderManifest(template, identity) {
  const escape = (value) => value.replaceAll("&", "&amp;").replaceAll('"', "&quot;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll("'", "&apos;");
  return template.replace(/\{\{([A-Z_]+)\}\}/g, (_, key) => {
    if (!identity[key]) throw new Error(`Missing manifest token ${key}`);
    return escape(identity[key]);
  });
}

export function distributionReady(directory, env) {
  return env.OCULUS_WINDOWS_DISTRIBUTION_APPROVED === "true"
    && ["FFMPEG-LICENSE.txt", "FFMPEG-README.txt", "FFMPEG-SOURCE-OFFER.txt"].every((file) => {
      const path = join(directory, file);
      return existsSync(path) && statSync(path).isFile() && readFileSync(path, "utf8").trim().length > 0;
    });
}

export function requireExecutable(path) {
  if (!existsSync(path) || !statSync(path).isFile() || statSync(path).size < 2) {
    throw new Error(`Missing or invalid Windows executable: ${path}`);
  }
  const descriptor = openSync(path, "r");
  const header = Buffer.alloc(2);
  try { readSync(descriptor, header, 0, 2, 0); } finally { closeSync(descriptor); }
  if (header.toString() !== "MZ") throw new Error(`Missing or invalid Windows executable: ${path}`);
}

function releaseSources(release, binaries) {
  const gui = join(release, "oculus-app.exe");
  if (!existsSync(gui)) {
    throw new Error(`Missing Tauri release GUI: ${gui}. Run tauri build on Windows; it renames Cargo's app.exe using mainBinaryName. A cargo-only build is not a packaged release.`);
  }
  const sources = {
    "oculus-app.exe": gui,
    "oculus.exe": join(release, "oculus.exe"),
    "oculus-keyd.exe": join(binaries, "oculus-keyd-x86_64-pc-windows-msvc.exe"),
    "ffmpeg.exe": join(binaries, "ffmpeg-x86_64-pc-windows-msvc.exe"),
  };
  for (const path of Object.values(sources)) requireExecutable(path);
  return sources;
}

export function validateWindowsRelease(release, binaries) {
  releaseSources(release, binaries);
  const nsis = join(release, "bundle", "nsis");
  const installers = existsSync(nsis) ? readdirSync(nsis).filter((file) => file.toLowerCase().endsWith(".exe")) : [];
  if (!installers.length) throw new Error(`Missing Windows NSIS installer in ${nsis}`);
  for (const file of installers) requireExecutable(join(nsis, file));
  return installers;
}

export function stageMsix({ content, release, binaries, distributionDir, assetsDir, template, identity }) {
  // Validate every input before replacing generated staging output.
  const sources = releaseSources(release, binaries);
  for (const file of ["FFMPEG-LICENSE.txt", "FFMPEG-README.txt"]) {
    if (!existsSync(join(distributionDir, file))) throw new Error(`Missing ffmpeg distribution notice: ${file}`);
  }
  if (!existsSync(join(assetsDir, "StoreLogo.scale-100.png"))) throw new Error(`Missing MSIX logos in ${assetsDir}; run bun run icons:windows`);
  const manifest = renderManifest(template, identity);
  rmSync(content, { recursive: true, force: true });
  mkdirSync(content, { recursive: true });
  for (const [name, source] of Object.entries(sources)) copyFileSync(source, join(content, name));
  for (const file of ["FFMPEG-LICENSE.txt", "FFMPEG-README.txt", "FFMPEG-SOURCE-OFFER.txt"]) {
    if (existsSync(join(distributionDir, file))) copyFileSync(join(distributionDir, file), join(content, file));
  }
  cpSync(assetsDir, join(content, "Assets"), { recursive: true });
  writeFileSync(join(content, "AppxManifest.xml"), manifest);
}

export function prepareReleaseConfig(rustDir) {
  const resources = JSON.parse(readFileSync(join(rustDir, "tauri.windows.conf.json"), "utf8")).bundle.resources;
  const offer = join(rustDir, "ffmpeg-notices", "FFMPEG-SOURCE-OFFER.txt");
  const config = { bundle: { resources: { ...resources } } };
  if (existsSync(offer)) config.bundle.resources["ffmpeg-notices/FFMPEG-SOURCE-OFFER.txt"] = "FFMPEG-SOURCE-OFFER.txt";
  const dest = join(rustDir, "target", "windows-release.conf.json");
  mkdirSync(dirname(dest), { recursive: true });
  writeFileSync(dest, JSON.stringify(config, null, 2) + "\n");
  return config;
}

function output(key, value) {
  if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `${key}=${value}\n`);
}

function summary(message) {
  console.log(message);
  if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, `${message}\n\n`);
}

export function main(env = process.env, args = process.argv.slice(2)) {
  if (args.includes("--check-release")) {
    const installers = validateWindowsRelease(join(rust, "target", "release"), join(rust, "binaries"));
    summary(`Validated Tauri Windows release executables and NSIS installer: ${installers.join(", ")}`);
    return;
  }
  if (args.includes("--check-distribution") || args.includes("--prepare-release")) {
    if (args.includes("--prepare-release")) prepareReleaseConfig(rust);
    const ready = distributionReady(distribution, env);
    output("distribution_ready", ready);
    if (!ready) summary("Windows installer artifacts are available for VM validation. Public Windows distribution and Store submission require FFMPEG-SOURCE-OFFER.txt plus OCULUS_WINDOWS_DISTRIBUTION_APPROVED=true, set once the ffmpeg source release is published and the offer reviewed; a successful build does not satisfy this release gate.");
    return;
  }
  const { identity, missing } = readMsixIdentity(env);
  output("msix_enabled", Boolean(identity));
  if (!identity) {
    summary(`MSIX packaging skipped. Configure the real Partner Center values: ${missing.join(", ")}. No package identity was invented.`);
    return;
  }
  if (args.includes("--check")) return;
  if (process.platform !== "win32") throw new Error("MSIX packaging requires Windows and the Windows SDK (MakePri.exe and MakeAppx.exe)");
  const release = join(rust, "target", "release");
  const out = join(release, "bundle", "msix");
  const content = join(out, "content");
  stageMsix({ content, release, binaries: join(rust, "binaries"), distributionDir: distribution, assetsDir: join(rust, "windows", "Assets"),
    template: readFileSync(join(rust, "windows", "AppxManifest.xml"), "utf8"), identity });
  const packaged = join(out, `Oculus_${identity.VERSION}_x64.msix`);
  const result = spawnSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-File", join(app, "scripts", "package-msix.ps1"),
    "-ContentDir", content, "-OutputPath", packaged], { stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`Windows SDK packaging failed (${result.status})`);
  summary(`Unsigned Store-upload MSIX created: ${packaged}. Installing it outside the Store requires separate signing. Build success does not approve Store submission.`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
