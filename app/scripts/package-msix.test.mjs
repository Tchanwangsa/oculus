import { afterEach, expect, test } from "bun:test";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { distributionReady, prepareReleaseConfig, readMsixIdentity, renderManifest, requireExecutable, stageMsix, validateWindowsRelease } from "./package-msix.mjs";

const roots = [];
function scratch() {
  const root = mkdtempSync(join(tmpdir(), "oculus-msix-"));
  roots.push(root);
  return root;
}
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });
const configured = {
  OCULUS_MSIX_IDENTITY_NAME: "Test.Oculus",
  OCULUS_MSIX_PUBLISHER: 'CN=Test & Partner, O="Publisher"',
  OCULUS_MSIX_DISPLAY_NAME: "Oculus & Coursework",
  OCULUS_MSIX_PUBLISHER_DISPLAY_NAME: "Test Publisher",
  OCULUS_MSIX_VERSION: "1.2.3.0",
};
const template = readFileSync(new URL("../src-tauri/windows/AppxManifest.xml", import.meta.url), "utf8");

test("missing Partner Center identity skips with the exact configuration requirements", () => {
  expect(readMsixIdentity({})).toEqual({ identity: null, missing: Object.keys(configured) });
  expect(readMsixIdentity({ ...configured, OCULUS_MSIX_PUBLISHER: "" }).missing).toEqual(["OCULUS_MSIX_PUBLISHER"]);
});

test("Store identity rejects bad names, missing distinguished names and invalid Store versions", () => {
  expect(readMsixIdentity(configured).identity.VERSION).toBe("1.2.3.0");
  for (const version of ["0.1.0", "0.1.0.0", "1.2.3.1", "1.2.65536.0", "1.2.3-beta.0", "1.02.3.0"]) {
    expect(() => readMsixIdentity({ ...configured, OCULUS_MSIX_VERSION: version })).toThrow("OCULUS_MSIX_VERSION");
  }
  expect(() => readMsixIdentity({ ...configured, OCULUS_MSIX_IDENTITY_NAME: "wrong name" })).toThrow("identity name");
  expect(() => readMsixIdentity({ ...configured, OCULUS_MSIX_PUBLISHER: "Test" })).toThrow("CN=");
});

test("manifest escapes identity text and separates GUI from console execution alias", () => {
  const xml = renderManifest(template, readMsixIdentity(configured).identity);
  expect(xml).not.toContain("{{");
  expect(xml).toContain('Publisher="CN=Test &amp; Partner, O=&quot;Publisher&quot;"');
  expect(xml).toContain('<DisplayName>Oculus &amp; Coursework</DisplayName>');
  expect(xml).toContain('Executable="oculus-app.exe"');
  expect(xml).toContain('Executable="oculus.exe" EntryPoint="Windows.FullTrustApplication"');
  expect(xml).toContain('<desktop:ExecutionAlias Alias="oculus.exe"');
  expect(xml).toContain('Capability Name="runFullTrust"');
  expect(xml).not.toContain("startupTask");
  expect(xml).not.toContain("unvirtualizedResources");
  expect(() => renderManifest("{{UNKNOWN}}", {})).toThrow("UNKNOWN");
});

function payload() {
  const root = scratch();
  const release = join(root, "release");
  const binaries = join(root, "binaries");
  const distributionDir = join(root, "distribution");
  const content = join(root, "content");
  for (const dir of [release, binaries, distributionDir, content]) mkdirSync(dir);
  for (const path of [join(release, "oculus-app.exe"), join(release, "oculus.exe"),
    join(binaries, "oculus-keyd-x86_64-pc-windows-msvc.exe"), join(binaries, "ffmpeg-x86_64-pc-windows-msvc.exe")]) writeFileSync(path, "MZfixture");
  for (const file of ["FFMPEG-LICENSE.txt", "FFMPEG-README.txt"]) writeFileSync(join(distributionDir, file), `Fixture notice: ${file}`);
  return { content, release, binaries, distributionDir, template, identity: readMsixIdentity(configured).identity };
}

test("payload stages all four executables together with distribution notices", () => {
  const input = payload();
  writeFileSync(join(input.content, "stale"), "old build");
  stageMsix(input);
  for (const exe of ["oculus-app.exe", "oculus.exe", "oculus-keyd.exe", "ffmpeg.exe"]) expect(readFileSync(join(input.content, exe), "utf8")).toBe("MZfixture");
  expect(readFileSync(join(input.content, "FFMPEG-README.txt"), "utf8")).toBe("Fixture notice: FFMPEG-README.txt");
  expect(readFileSync(join(input.content, "AppxManifest.xml"), "utf8")).toContain('Version="1.2.3.0"');
});

test("a placeholder executable fails before removing a previous staged payload", () => {
  const input = payload();
  writeFileSync(join(input.content, "last-good"), "keep");
  writeFileSync(join(input.release, "oculus.exe"), "");
  expect(() => stageMsix(input)).toThrow("Windows executable");
  expect(readFileSync(join(input.content, "last-good"), "utf8")).toBe("keep");
  expect(() => requireExecutable(join(input.release, "does-not-exist.exe"))).toThrow("Windows executable");
});

test("cargo-only GUI output is rejected before replacing a previous staged payload", () => {
  const input = payload();
  rmSync(join(input.release, "oculus-app.exe"));
  writeFileSync(join(input.release, "app.exe"), "MZcargo-only");
  writeFileSync(join(input.content, "last-good"), "keep");
  expect(() => stageMsix(input)).toThrow("Run tauri build on Windows");
  expect(readFileSync(join(input.content, "last-good"), "utf8")).toBe("keep");
});

test("release validation requires an actual NSIS executable in addition to payload and notices", () => {
  const input = payload();
  expect(() => validateWindowsRelease(input.release, input.binaries)).toThrow("Missing Windows NSIS installer");
  const nsis = join(input.release, "bundle", "nsis");
  mkdirSync(nsis, { recursive: true });
  writeFileSync(join(nsis, "Oculus_1.2.3_x64-setup.exe"), "MZinstaller");
  expect(validateWindowsRelease(input.release, input.binaries)).toEqual(["Oculus_1.2.3_x64-setup.exe"]);
  writeFileSync(join(nsis, "Oculus_1.2.3_x64-setup.exe"), "bad installer");
  expect(() => validateWindowsRelease(input.release, input.binaries)).toThrow("Windows executable");
});

test("public distribution requires a written offer plus explicit review approval", () => {
  const input = payload();
  const approved = { OCULUS_WINDOWS_DISTRIBUTION_APPROVED: "true" };
  expect(distributionReady(input.distributionDir, approved)).toBe(false);
  writeFileSync(join(input.distributionDir, "FFMPEG-SOURCE-OFFER.txt"), "Fixture offer, for the test only");
  expect(distributionReady(input.distributionDir, {})).toBe(false);
  expect(distributionReady(input.distributionDir, approved)).toBe(true);
  stageMsix(input);
  expect(readFileSync(join(input.content, "FFMPEG-SOURCE-OFFER.txt"), "utf8")).toContain("Fixture offer");
});

test("release config includes an available offer while preserving the upstream notices", () => {
  const root = scratch();
  mkdirSync(join(root, "windows", "distribution"), { recursive: true });
  writeFileSync(join(root, "tauri.windows.conf.json"), JSON.stringify({ mainBinaryName: "oculus-app", bundle: { resources: { "windows/distribution/FFMPEG-LICENSE.txt": "FFMPEG-LICENSE.txt" } } }));
  expect(prepareReleaseConfig(root).bundle.resources).toEqual({ "windows/distribution/FFMPEG-LICENSE.txt": "FFMPEG-LICENSE.txt" });
  // This overlay inherits mainBinaryName from the platform config loaded by Tauri.
  expect(prepareReleaseConfig(root).mainBinaryName).toBeUndefined();
  writeFileSync(join(root, "windows", "distribution", "FFMPEG-SOURCE-OFFER.txt"), "Fixture offer");
  expect(prepareReleaseConfig(root).bundle.resources["windows/distribution/FFMPEG-SOURCE-OFFER.txt"]).toBe("FFMPEG-SOURCE-OFFER.txt");
});

test("native app manifest opts into long paths without requesting elevation", () => {
  const xml = readFileSync(new URL("../src-tauri/windows-app-manifest.xml", import.meta.url), "utf8");
  expect(xml).toContain('xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings"');
  expect(xml).toMatch(/longPathAware[^>]*>true<\/longPathAware>/);
  expect(xml).not.toContain("requireAdministrator");
});

test("Windows bundle keeps custom chrome, separate GUI and CLI names, and all required sidecars", () => {
  const config = JSON.parse(readFileSync(new URL("../src-tauri/tauri.windows.conf.json", import.meta.url), "utf8"));
  expect(config.mainBinaryName).toBe("oculus-app");
  expect(config.app.windows).toHaveLength(1);
  expect(config.app.windows[0].decorations).toBe(false);
  for (const macOnly of ["titleBarStyle", "trafficLightPosition", "hiddenTitle"]) expect(config.app.windows[0][macOnly]).toBeUndefined();
  expect(config.bundle.targets).toContain("nsis");
  expect(config.bundle.windows.nsis.installMode).toBe("currentUser");
  expect(config.bundle.externalBin.slice().sort()).toEqual(["binaries/ffmpeg", "binaries/oculus", "binaries/oculus-keyd"]);
  expect(config.app.security.assetProtocol.scope).toEqual(["$HOME/Oculus/courses/**", "$HOME/Oculus/lectures/**", "$HOME/Oculus/agents/**"]);
});
