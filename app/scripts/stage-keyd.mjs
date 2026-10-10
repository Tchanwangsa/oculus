// Build the bundled oculus-keyd and stage it as the `externalBin` sidecar:
// the release policy (no `dev` feature), signed as keyd-build.mjs does,
// written to `binaries/oculus-keyd-<triple>`. Tauri copies it to
// `Contents/MacOS/oculus-keyd` and signs it with the rest of the bundle.
// `--out <dir>` stages somewhere else instead.
import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import { buildKeyd, log } from "./keyd-build.mjs";
import { binaries, stageKeyd } from "./runtime.mjs";

const flag = process.argv.indexOf("--out");
const out = flag === -1 ? binaries : resolve(process.argv[flag + 1] ?? "");
if (flag !== -1 && !process.argv[flag + 1]) throw new Error("--out needs a directory");

const built = buildKeyd("bundle");
if (!built) process.exit(0);

// The signature must hold before it is shipped, and the stage must be this
// build — not whatever was staged before.
execFileSync("codesign", ["--verify", "--strict", built.binary], { stdio: "inherit" });
const dest = stageKeyd(built.binary, out);
const staged = execFileSync(dest, ["source-hash"], { encoding: "utf8" }).trim();
if (staged !== built.hash) throw new Error(`${dest} reports source ${staged}, expected ${built.hash}`);
log(`staged ${dest}`);
