import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { app } from "./runtime.mjs";

const ci = Bun.YAML.parse(readFileSync(join(app, "..", ".github/workflows/ci.yml"), "utf8"));
const filters = Bun.YAML.parse(ci.jobs.changes.steps.find((step) => step.id === "filter").with.filters);
const selected = (name, file) => filters[name].some((pattern) => new Bun.Glob(pattern).match(file));

test("frontend and docs changes avoid native compilation", () => {
  for (const file of ["app/src/pages/Settings.tsx", "app/src/index.css", "app/tests/editor.test.ts", "docs/development.md"]) {
    expect(selected("native", file), file).toBe(false);
    expect(selected("studio", file), file).toBe(false);
  }
});

test("native source, shared dependencies and build inputs select both platform jobs", () => {
  for (const file of [
    "app/src-tauri/src/lib.rs",
    "app/src-tauri/vendor/librespot-connect/src/spirc.rs",
    "app/src-tauri/tauri.windows.conf.json",
    "app/keyd/core/src/platform/windows.rs",
    "app/studio-core/src/lib.rs",
    "app/math-core/katex/src/lib.rs",
    "app/canvas-core/src/lib.rs",
    "app/scripts/stage-cli.mjs",
    "app/package.json",
    "app/bun.lock",
    "app/patches/example.patch",
    "app/src-tauri/.cargo/config.toml",
    "rust-toolchain.toml",
    ".github/workflows/ci.yml",
    ".github/workflows/release.yml",
    "docs/cli-reference.md",
  ]) expect(selected("native", file), file).toBe(true);

  for (const name of ["check", "windows"]) {
    expect(ci.jobs[name].if).toContain("needs.changes.outputs.native == 'true'");
    expect(ci.jobs[name].if).toContain("github.event_name == 'workflow_dispatch'");
  }
});

test("standalone checks include their transitive inputs", () => {
  expect(selected("keyd", "app/keyd/core/src/lib.rs")).toBe(true);
  expect(selected("editor", "app/src/components/editor/noteLanguage.ts")).toBe(true);
  expect(selected("editor", "app/bun.lock")).toBe(true);
  expect(selected("studio", "app/math-core/katex/data/symbols.json")).toBe(true);
  for (const name of Object.keys(filters)) {
    expect(selected(name, ".github/workflows/ci.yml"), name).toBe(true);
    expect(selected(name, "app/.cargo/config.toml"), name).toBe(true);
  }
});

test("native jobs use the verified frontend artifact before compiling Rust", () => {
  const upload = ci.jobs.frontend.steps.find((step) => step.uses?.startsWith("actions/upload-artifact@"));
  expect(upload.with.path).toBe("app/dist");
  expect(upload.with["if-no-files-found"]).toBe("error");
  for (const name of ["check", "windows"]) {
    const job = ci.jobs[name];
    expect(job.needs).toContain("frontend");
    const download = job.steps.findIndex((step) => step.uses?.startsWith("actions/download-artifact@"));
    const compile = job.steps.findIndex((step) => step.run === "bun run stage-cli");
    expect(download).toBeGreaterThan(-1);
    expect(download).toBeLessThan(compile);
    expect(job.steps[download].with.name).toBe(upload.with.name);
    expect(job.steps[download].with.path).toBe("app/dist");
  }
});


test("routine CI leaves installer packaging to the release workflow", () => {
  for (const job of Object.values(ci.jobs)) {
    for (const step of job.steps ?? []) {
      expect(step.run ?? "").not.toMatch(/tauri build|package-msix/);
      expect(step.uses ?? "").not.toContain("tauri-apps/tauri-action@");
    }
  }
});
