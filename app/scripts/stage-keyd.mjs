// Build the bundled oculus-keyd and stage its helper app for the bundle: the
// release policy (no `dev` feature), signed as keyd-build.mjs does, written to
// `binaries/Oculus Helper.app`. `bundle.macOS.files` in tauri.macos.conf.json
// copies it to `Contents/Helpers/`, and Tauri seals it into the app without
// re-signing it. `--out <dir>` stages somewhere else instead.
import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import { buildKeyd, log } from "./keyd-build.mjs";
import { binaries, helperProgram, stageKeyd } from "./runtime.mjs";

const flag = process.argv.indexOf("--out");
const out = flag === -1 ? binaries : resolve(process.argv[flag + 1] ?? "");
if (flag !== -1 && !process.argv[flag + 1]) throw new Error("--out needs a directory");

const built = buildKeyd("bundle");
if (!built) process.exit(0);

// The signature must hold before it is shipped and after the copy, and the
// stage must be this build — not whatever was staged before.
execFileSync("codesign", ["--verify", "--strict", built.helper], { stdio: "inherit" });
const dest = stageKeyd(built.helper, out);
execFileSync("codesign", ["--verify", "--strict", dest], { stdio: "inherit" });
const staged = execFileSync(helperProgram(dest), ["source-hash"], { encoding: "utf8" }).trim();
if (staged !== built.hash) throw new Error(`${dest} reports source ${staged}, expected ${built.hash}`);
log(`staged ${dest}`);
