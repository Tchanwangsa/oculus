import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { PickerProvider } from "@/lib/harness";

// The picker's import graph reaches modules that listen on `window` at load.
const globals = globalThis as { window?: unknown; document?: unknown };
const before = { window: globals.window, document: globals.document };
const inert = { addEventListener: () => {}, removeEventListener: () => {} };
globals.window ??= inert;
globals.document ??= { ...inert, documentElement: { style: {} } };
const { ModelPicker } = await import("@/components/harness/composer/ModelPicker");
Object.assign(globals, before);

const noop = () => {};
const providers: PickerProvider[] = [
  { id: "codex", label: "Codex", models: [{ id: "gpt-6.1-sol", label: "GPT-6.1 Sol", description: "", reasoningEfforts: ["medium", "high"], defaultReasoningEffort: "medium" }], loading: false, health: "ok" },
] as PickerProvider[];

const render = (switchFrom: "claude" | null) =>
  renderToStaticMarkup(
    <ModelPicker providers={providers} provider="codex" model="gpt-6.1-sol" reasoning="high" switchFrom={switchFrom}
      onProvider={noop} onModel={noop} onReasoning={noop} />,
  );

test("the trigger names the pick the next message will use, and says it waits for that message", () => {
  const pending = render("claude");
  expect(pending).toContain("GPT-6.1 Sol");
  expect(pending).toContain("High");
  expect(pending).toContain('title="Codex · gpt-6.1-sol · High reasoning · from your next message"');
  expect(render(null)).toContain('title="Codex · gpt-6.1-sol · High reasoning"');
});
