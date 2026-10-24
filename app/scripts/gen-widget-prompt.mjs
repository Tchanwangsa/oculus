// Bun evaluates the component schemas; browser globals are not used by the printer.
import { readFileSync, writeFileSync } from "node:fs";
for (const name of ["window", "document"]) {
  globalThis[name] ??= Object.assign(new EventTarget(), {
    documentElement: { style: {} },
  });
}
const { generateOculusUiPrompt } = await import(
  "../src/components/markdown/openui/prompt.ts"
);
const path = new URL(
  "../src-tauri/templates/skills/oculus-ui/SKILL.md",
  import.meta.url,
);
const current = readFileSync(path, "utf8");
const frontmatter = current.slice(0, current.indexOf("\n---", 3) + 4);
writeFileSync(
  path,
  `${frontmatter}\n\n# Chat widgets and proposal cards\n\n${generateOculusUiPrompt()}\n`,
);
console.log("[docs] refreshed oculus-ui signatures and examples");
