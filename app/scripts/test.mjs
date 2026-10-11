// Discover script tests without depending on the host shell's glob expansion.
import { readdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
const scripts = readdirSync(new URL("./", import.meta.url)).filter((name) => name.endsWith(".test.mjs")).sort().map((name) => `scripts/${name}`);
const result = spawnSync("bun", ["test", "tests", ...scripts], { stdio: "inherit", windowsHide: true });
if (result.error) { console.error(result.error.message); process.exit(1); }
process.exit(result.status ?? 1);
