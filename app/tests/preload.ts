import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { loadMaths } from "@/lib/maths";

// r3f has no `exports` map, so bun loads its CommonJS build, whose
// `require("three")` fails once a test has imported three as an ES module.
// Its ES build is what the widget kit's bundler uses anyway.
Bun.plugin({
  name: "r3f-es-build",
  setup(build) {
    build.onLoad({ filter: /@react-three\/fiber\/dist\/react-three-fiber\.cjs\.js$/ }, () => ({
      contents: 'export * from "./react-three-fiber.esm.js";',
      loader: "js",
    }));
  },
});

const wasm = fileURLToPath(new URL("../math-core/pkg/oculus_math_bg.wasm", import.meta.url));
if (!existsSync(wasm)) {
  throw new Error(`${wasm} is missing: run \`bun run math\` in app/ (\`bun run test\` does it first)`);
}
await loadMaths();
