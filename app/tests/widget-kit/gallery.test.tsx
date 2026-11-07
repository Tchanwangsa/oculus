import { describe, expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { createElement, type ComponentType } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { buildSrcdoc, escapeScript, importMap } from "@/components/markdown/widget/srcdoc";
import { widgetTitle } from "@/components/markdown/widget/title";
import * as UI from "../../widget-kit/ui";
import * as State from "../../widget-kit/state";
import * as Plane2d from "../../widget-kit/2d";
import * as Maths from "../../widget-kit/math";
import * as Space from "../../widget-kit/3d";
import * as Ask from "../../widget-kit/ask";

// The oculus-widgets skill's gallery (src-tauri/templates/skills/), and each
// example's real transpile output, which a Rust test keeps equal
// (agents/tests.rs, `widget_gallery_fixtures_are_the_transpile_output`).
const GALLERY = join(import.meta.dir, "../../src-tauri/templates/skills/oculus-widgets");
const FIXTURES = join(import.meta.dir, "../fixtures/widget-gallery");
const KIT = "http://127.0.0.1:4321/widget-kit/";
const examples = readdirSync(GALLERY)
  .filter((f) => f.endsWith(".tsx"))
  .sort();

// The examples import the kit by its import-map names, as a frame does.
const kit: Record<string, object> = {
  "oculus/ui": UI,
  "oculus/state": State,
  "oculus/2d": Plane2d,
  "oculus/math": Maths,
  "oculus/3d": Space,
  "oculus/ask": Ask,
};
Bun.plugin({
  name: "oculus-kit-names",
  setup(build) {
    for (const [name, exports] of Object.entries(kit)) {
      build.module(name, () => ({ exports: { ...exports }, loader: "object" }));
    }
  },
});

test("the gallery is the four examples the skill names", () => {
  expect(examples).toEqual(["bloch-sphere.tsx", "bubble-sort.tsx", "derivative-steps.tsx", "vector-field.tsx"]);
  const skill = readFileSync(join(GALLERY, "SKILL.md"), "utf8");
  for (const file of examples) expect(skill).toContain(`**\`${file}\`**`);
});

describe.each(examples)("%s", (file) => {
  const source = readFileSync(join(GALLERY, file), "utf8");
  const js = readFileSync(join(FIXTURES, file.replace(/\.tsx$/, ".js")), "utf8");

  test("names itself on its first line", () => {
    expect(widgetTitle(source)).not.toBe("Widget");
  });

  test("its transpiled module builds a frame document that mounts it", () => {
    const doc = buildSrcdoc({ kitBase: KIT, nonce: "n", js, state: {}, tokens: {}, dark: false });
    expect(doc).toContain(escapeScript(js));
    expect(js).toContain("const __oculusWidget = ");
    expect(js).not.toContain("export default");
    expect(doc).toContain("__oculusMount(__oculusWidget);");
    const imported = [...js.matchAll(/^import .* from "([^"]+)";$/gm)].map((m) => m[1]);
    expect(imported.length).toBeGreaterThan(0);
    for (const name of imported) expect(Object.keys(importMap(KIT).imports)).toContain(name);
  });

  test("renders on the kit", async () => {
    const mod = (await import(join(GALLERY, file))) as { default: ComponentType };
    const html = renderToStaticMarkup(createElement(mod.default));
    expect(html).toStartWith('<div class="o-widget"');
  });
});
