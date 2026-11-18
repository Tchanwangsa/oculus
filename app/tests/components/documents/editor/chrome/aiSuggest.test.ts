import { describe, expect, mock, test } from "bun:test";
import { EditorState, type TransactionSpec } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

// The real module drags in the app shell, which needs `window`.
mock.module("@/components/documents/DocumentControls", () => ({ SUGGEST_IDLE: { pending: false, error: null } }));
const { ghostBindings, ghostField, setGhost } = await import("@/components/documents/editor/chrome/aiSuggest");

// A DOM-less stand-in: the bindings only read `state` and call `dispatch`.
function fakeView(ghost: boolean) {
  let state = EditorState.create({ doc: "Hello ", selection: { anchor: 6 }, extensions: [ghostField] });
  if (ghost) state = state.update({ effects: setGhost.of({ pos: 6, text: "world today" }) }).state;
  const view = {
    get state() { return state; },
    dispatch(spec: TransactionSpec) { state = state.update(spec).state; },
  };
  return view as unknown as EditorView & { state: EditorState };
}
const run = (key: string, view: EditorView) => ghostBindings.find((b) => b.key === key)!.run!(view);

describe("AI suggestion keys", () => {
  test("→ accepts the whole ghost exactly like Tab", () => {
    const tab = fakeView(true);
    const right = fakeView(true);
    expect(run("Tab", tab)).toBe(true);
    expect(run("ArrowRight", right)).toBe(true);
    expect(right.state.doc.toString()).toBe("Hello world today");
    expect(right.state.doc.toString()).toBe(tab.state.doc.toString());
    expect(right.state.selection.main.head).toBe(tab.state.selection.main.head);
    expect(right.state.field(ghostField)).toBeNull();
  });

  test("→ without a ghost passes through and changes nothing", () => {
    const view = fakeView(false);
    for (let i = 0; i < 3; i++) expect(run("ArrowRight", view)).toBe(false);
    expect(view.state.doc.toString()).toBe("Hello ");
    expect(view.state.field(ghostField)).toBeNull();
  });
});
