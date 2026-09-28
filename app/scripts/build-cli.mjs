import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, rmSync, symlinkSync } from "node:fs";
import { dirname, join } from "node:path";
import { homedir } from "node:os";
import { fileURLToPath } from "node:url";

const app = dirname(dirname(fileURLToPath(import.meta.url)));
const exe = process.platform === "win32" ? ".exe" : "";
const debug = process.argv.includes("--debug");
const binary = join(app, "src-tauri", "target", debug ? "debug" : "release", `oculus${exe}`);
// Stage-cli handles the Tauri externalBin bootstrap on a clean checkout.
execFileSync(process.execPath, [join(app, "scripts", "fetch-ffmpeg.mjs")], { stdio: "inherit", windowsHide: true });
execFileSync(process.execPath, [join(app, "scripts", "fetch-pdfium.mjs")], { stdio: "inherit", windowsHide: true });
execFileSync(process.execPath, [join(app, "scripts", "stage-cli.mjs"), ...(debug ? ["--debug"] : [])], { stdio: "inherit", windowsHide: true });
if (process.argv.includes("--install")) {
  const bin = join(homedir(), ".local", "bin");
  const dest = join(bin, `oculus${exe}`);
  mkdirSync(bin, { recursive: true });
  rmSync(dest, { force: true });
  if (exe) copyFileSync(binary, dest);
  else symlinkSync(binary, dest);
  execFileSync(dest, ["docs"], { stdio: "inherit", windowsHide: true });
  console.log(`[cli] installed ${dest}; ensure ${bin} is on PATH`);
}
