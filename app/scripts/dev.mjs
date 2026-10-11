// Pass the CLI watch flag through env on every shell, including cmd.exe.
import { spawn } from "node:child_process";
const child = spawn("bun", ["run", "dev"], {
  stdio: "inherit",
  env: { ...process.env, OCULUS_CLI_WATCH: "1" },
  windowsHide: true,
});
child.on("error", (error) => { console.error(error.message); process.exit(1); });
child.on("exit", (code) => process.exit(code ?? 1));
for (const signal of ["SIGINT", "SIGTERM"]) process.on(signal, () => child.kill(signal));
