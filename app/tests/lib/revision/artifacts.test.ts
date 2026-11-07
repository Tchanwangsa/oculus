import { afterAll, expect, mock, test } from "bun:test";
import { artifactInput } from "@/lib/revision/artifacts";
import { readArtifact } from "@/lib/revision/reads";
import type { RevisionArtifact } from "@/lib/revision/types";
import { createWidgetStore, entryState } from "@/stores/chat/widgetStore";

const calls: { cmd: string; args: unknown }[] = [];
let answer: () => Promise<unknown> = async () => 1;
mock.module("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args: unknown) => {
    calls.push({ cmd, args });
    return answer();
  },
}));
// `notifyRevisionUpdated` dispatches on `window`; tests run without a DOM.
const globals = globalThis as { window?: unknown };
const hadWindow = "window" in globals;
globals.window ??= new EventTarget();
afterAll(() => {
  // Module mocks outlive the file; leave later files an invoke that fails as it does without Tauri.
  answer = async () => { throw new Error("Tauri is not available"); };
  if (!hadWindow) delete globals.window;
});
const { createArtifact, updateArtifact } = await import("@/lib/revision/writes");

const WIDGET = "// title: Phase\nexport default () => null;";

test("Save to Revision on a widget fence writes its source, state and format, never its Allow", () => {
  const store = createWidgetStore();
  store.getState().save("4:12:1", WIDGET, { angle: 0.5 });
  store.getState().setAsk("4:12:1", WIDGET, { allowed: true, used: 6 });
  const entry = store.getState().entries["4:12:1"];
  expect(entry?.ask).toEqual({ allowed: true, used: 6 });
  const input = artifactInput({
    title: "  Phase  ",
    program: WIDGET,
    format: "widget",
    state: entryState(entry, WIDGET),
    subject: 3,
    row: { thread: 4, row: 12 },
  });
  expect(input).toEqual({
    title: "Phase",
    program: WIDGET,
    format: "widget",
    state: { angle: 0.5 },
    subject_id: 3,
    source_ref: "4:12",
  });
  expect(JSON.stringify(input)).not.toContain("allowed");
  expect(artifactInput({ ...input, title: "x", program: WIDGET, subject: null, row: null, state: {} }).source_ref).toBeNull();
});

test("createArtifact and updateArtifact carry the format to Rust's writer", async () => {
  calls.length = 0;
  const input = artifactInput({ title: "Phase", program: WIDGET, format: "widget", state: { a: 1 }, subject: 3, row: null });
  expect(await createArtifact(input)).toBe(1);
  await updateArtifact(1, { state: { a: 2 } });
  expect(calls).toEqual([
    { cmd: "revision_create_entity", args: { entity: "artifact", input } },
    { cmd: "revision_update_entity", args: { entity: "artifact", id: 1, patch: { state: { a: 2 } } } },
  ]);
});

test("reads draw a widget row as a widget and anything else as OpenUI", () => {
  const row = { id: 1, title: "A", program: "x", state: {} } as RevisionArtifact;
  expect(readArtifact({ ...row, format: "widget" }).format).toBe("widget");
  expect(readArtifact({ ...row, format: "openui" }).format).toBe("openui");
  expect(readArtifact({ ...row, format: "html" as never }).format).toBe("openui");
  expect(readArtifact({ ...row, format: undefined as never }).format).toBe("openui");
});
