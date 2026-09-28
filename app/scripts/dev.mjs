// Use an environment object so the same Tauri dev hook works on Windows
// and Unix without shell-specific `NAME=value command` syntax.
import { spawn } from "node:child_process";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";

const child = spawn("bun", ["run", "dev"], {
  cwd: dirname(dirname(fileURLToPath(import.meta.url))),
  env: { ...process.env, OCULUS_CLI_WATCH: "1" },
  stdio: "inherit",
  windowsHide: true,
});
child.on("error", (error) => {
  console.error(`[dev] ${error.message}`);
  process.exitCode = 1;
});
child.on("exit", (code) => { process.exitCode = code ?? 1; });
for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => child.kill(signal));
}
