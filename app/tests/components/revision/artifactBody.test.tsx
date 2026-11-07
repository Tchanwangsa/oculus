import { afterAll, expect, test } from "bun:test";
import { isValidElement, type ReactElement } from "react";
import type { RevisionArtifact } from "@/lib/revision/types";

// The widget's imports reach modules that listen on `window` and `document`
// at load; tests run without a DOM (as in widget.test.tsx).
const globals = globalThis as { window?: unknown; document?: unknown };
const stubbed = (["window", "document"] as const).filter((k) => !(k in globals));
for (const k of stubbed) globals[k] = Object.assign(new EventTarget(), { documentElement: { style: {} } });
afterAll(() => {
  for (const k of stubbed) delete globals[k];
});
const { ArtifactBody, artifactFence } = await import("@/components/revision/ArtifactBody");
const { LiveWidget } = await import("@/components/markdown/widget/LiveWidget");

const artifact = (format: RevisionArtifact["format"], program: string): RevisionArtifact => ({
  id: 7,
  topic_ids: [],
  subject_id: 1,
  title: "Phase",
  program,
  format,
  state: { angle: 1 },
  source_ref: "4:12",
  created_at: "",
  updated_at: "",
});
const persist = () => {};

test("a widget artifact runs in LiveWidget under its own key, without Save", () => {
  const program = "// title: Phase\nexport default () => null;";
  const el = ArtifactBody({ artifact: artifact("widget", program), onPersistState: persist }) as ReactElement<Record<string, unknown>>;
  expect(el.type).toBe(LiveWidget);
  expect(el.props).toMatchObject({
    source: program,
    storeKey: "artifact:7",
    savedState: { angle: 1 },
    onPersistState: persist,
    hideSave: true,
    md: "```widget\n" + program + "\n```",
  });
});

test("an OpenUI artifact runs in the lazy OpenUI block", () => {
  const el = ArtifactBody({ artifact: artifact("openui", "root = Stack([])"), onPersistState: persist }) as ReactElement<{ children: ReactElement<Record<string, unknown>> }>;
  expect(el.type).not.toBe(LiveWidget);
  const block = el.props.children;
  expect(isValidElement(block)).toBe(true);
  expect(block.props).toMatchObject({ source: "root = Stack([])", savedState: { angle: 1 }, onPersistState: persist, hideSave: true });
});

test("Ask about this quotes the artifact in its own fence", () => {
  expect(artifactFence(artifact("widget", "x"))).toBe("```widget\nx\n```");
  expect(artifactFence(artifact("openui", "y"))).toBe("```openui\ny\n```");
});
