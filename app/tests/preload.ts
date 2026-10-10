import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { loadMaths } from "@/lib/maths";

const wasm = fileURLToPath(new URL("../math-core/pkg/oculus_math_bg.wasm", import.meta.url));
if (!existsSync(wasm)) {
  throw new Error(`${wasm} is missing: run \`bun run math\` in app/ (\`bun run test\` does it first)`);
}
await loadMaths();
