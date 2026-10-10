import { describe, expect, test } from "bun:test";
import { CompletionContext } from "@codemirror/autocomplete";
import { EditorState } from "@codemirror/state";

import { noteMarkdown } from "@/components/documents/editor/core/language";
import { focusedField, setFocused } from "@/components/documents/editor/core/liveFocus";
import { visualMath, visualMathField } from "@/components/documents/editor/math/field/mathField";
import { pendingHtml } from "@/components/documents/editor/math/field/mathView/pending";
import { listKey } from "@/components/documents/editor/math/field/mathView/popover/keys";
import { fieldHtml } from "@/components/documents/editor/math/field/mathView/render";
import { commandOptions } from "@/components/documents/editor/math/field/mathView/popover/options";
import { GAP, placeList } from "@/components/documents/editor/math/field/mathView/popover/place";
import { mathCompletionSource } from "@/components/documents/editor/math/tools/mathTools/completion";
import { fieldTemplate } from "@/components/documents/editor/math/tools/mathPalette";
import { MathField, type FieldCommand } from "@/lib/maths";

const key = (k: string, mods: Partial<Record<"shiftKey" | "metaKey" | "ctrlKey" | "altKey", boolean>> = {}) => ({
  key: k,
  shiftKey: false,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  ...mods,
});

describe("the \\command list's options", () => {
  test("\\sqrt offers the plain root and the root with an index, plain first", () => {
    expect(commandOptions("sqrt").map((o) => o.template)).toEqual(["\\sqrt{#{}}", "\\sqrt[#{}]{#{}}"]);
    expect(commandOptions("sqrt").map((o) => o.label)).toEqual(["\\sqrt{}", "\\sqrt[]{}"]);
  });

  test("the command typed exactly comes first, its bare form before its templates", () => {
    expect(commandOptions("in")[0].template).toBe("\\in");
    expect(commandOptions("int")[0].template).toBe("\\int");
    expect(commandOptions("alp")[0].name).toBe("\\alpha");
    expect(commandOptions("fra")[0].template).toBe("\\frac{#{}}{#{}}");
  });

  test("nothing right after \\ or for a name nothing starts with", () => {
    expect(commandOptions("")).toEqual([]);
    expect(commandOptions("zzz")).toEqual([]);
  });

  test("each template once", () => {
    const templates = commandOptions("h").map((o) => o.template);
    expect(new Set(templates).size).toBe(templates.length);
  });
});

describe("the \\command list's keys", () => {
  test("↑/↓ move, Space, Tab and Enter accept", () => {
    expect(listKey(key("ArrowDown"), 3)).toEqual({ move: 1 });
    expect(listKey(key("ArrowUp"), 3)).toEqual({ move: -1 });
    for (const k of [" ", "Tab", "Enter"]) expect(listKey(key(k), 3)).toBe("accept");
  });

  test("an empty list, modifiers and other keys go to the model", () => {
    expect(listKey(key("Enter"), 0)).toBeNull();
    expect(listKey(key("ArrowDown"), 0)).toBeNull();
    expect(listKey(key("Tab", { shiftKey: true }), 3)).toBeNull();
    expect(listKey(key("Enter", { metaKey: true }), 3)).toBeNull();
    expect(listKey(key("Escape"), 3)).toBeNull();
    expect(listKey(key("Backspace"), 3)).toBeNull();
    expect(listKey(key("a"), 3)).toBeNull();
  });
});

describe("the \\command list's place", () => {
  const bounds = { left: 0, top: 0, right: 800, bottom: 600 };
  const size = { width: 200, height: 150 };
  const caretAt = (x: number, y: number) => ({ left: x, right: x, top: y, bottom: y + 20 });

  test("under the caret, its left edge at the caret's", () => {
    expect(placeList(caretAt(100, 100), size, bounds)).toEqual({ left: 100, top: 120 + GAP, above: false });
  });

  test("above the caret when it would pass the bottom", () => {
    expect(placeList(caretAt(100, 500), size, bounds)).toEqual({ left: 100, top: 500 - GAP - 150, above: true });
  });

  test("stays under when there is less room above", () => {
    const short = { ...bounds, bottom: 220 };
    expect(placeList(caretAt(100, 60), size, short).above).toBe(false);
  });

  test("slides left to stay inside the right edge, and never past the left", () => {
    expect(placeList(caretAt(700, 100), size, bounds).left).toBe(600);
    expect(placeList(caretAt(-20, 100), size, bounds).left).toBe(0);
    expect(placeList(caretAt(50, 100), size, { ...bounds, left: 80 }).left).toBe(80);
  });
});

describe("accepting an option in the field", () => {
  const typed = (field: MathField, text: string) => {
    for (const c of text) field = field.run({ insert: c }).field;
    return field;
  };
  const run = (field: MathField, ...commands: FieldCommand[]) => {
    for (const c of commands) field = field.run(c).field;
    return field;
  };
  const caret = (field: MathField) => field.stops[field.head];

  test("\\sqrt with an index puts the caret in the index, then Tab or → reach the radicand", () => {
    const pending = typed(MathField.open("x", false), "\\sqrt");
    expect(pending.mode).toBe("command");
    expect(pending.pending).toBe("sqrt");
    const root = run(pending, { template: fieldTemplate("\\sqrt[#{}]{#{}}") });
    expect(root.source).toBe("x\\sqrt[]{}");
    expect(root.mode).toBe("math");
    expect(root.pending).toBeUndefined();
    expect(caret(root)).toBe("x\\sqrt[".length);
    expect(caret(run(root, "tab"))).toBe("x\\sqrt[]{".length);
    expect(caret(run(root, { right: { extend: false } }))).toBe("x\\sqrt[]{".length);
  });

  test("the plain root and a text command", () => {
    const root = run(typed(MathField.open("", false), "\\sqrt"), { template: fieldTemplate("\\sqrt{#{}}") });
    expect(root.source).toBe("\\sqrt{}");
    expect(caret(root)).toBe("\\sqrt{".length);
    const text = run(typed(MathField.open("", false), "\\tex"), { template: fieldTemplate("\\text{#{}}") });
    expect(text.source).toBe("\\text{}");
    expect(text.mode).toBe("text");
  });
});

describe("\\ completion and the visual field", () => {
  const doc = "Text $x+\\alpha$ end";
  const pos = doc.indexOf("$ end");
  const complete = (state: EditorState) => mathCompletionSource(new CompletionContext(state, pos, false));

  test("stays shut over maths the field is open on", () => {
    let state = EditorState.create({
      doc,
      selection: { anchor: pos },
      extensions: [noteMarkdown(), focusedField, visualMathField],
    });
    state = state.update({ effects: setFocused.of(true) }).state;
    expect(visualMath(state)).not.toBeNull();
    expect(complete(state)).toBeNull();
  });

  test("opens on maths typed as source (Raw mode)", () => {
    const state = EditorState.create({ doc, selection: { anchor: pos }, extensions: [noteMarkdown()] });
    expect(complete(state)?.from).toBe(doc.indexOf("\\alpha"));
  });
});

describe("the pending \\name in the rendering", () => {
  const ranges = (html: string) => [...html.matchAll(/data-s="(\d+)" data-e="(\d+)"/g)].map((m) => `${m[1]}-${m[2]}`);
  const pendingAt = (source: string, offset: number, last = false) => {
    const field = MathField.open(source, false);
    const ids = [...field.stops].flatMap((s, id) => (s === offset ? [id] : []));
    const id = last ? ids.at(-1)! : ids[0];
    return field.select(id, id).withPending("sin");
  };

  test("is typed in at the caret, with no range, the rest keeping theirs", () => {
    const html = pendingHtml(pendingAt("a+b", 1), false)!;
    expect(html).toContain("data-pending");
    expect(html).toContain("\\sin");
    expect(ranges(html)).toEqual(ranges(fieldHtml("a+b", false)));
  });

  test("in a bare script, the script's own ranges are kept", () => {
    const html = pendingHtml(pendingAt("x^2", 3), false)!;
    expect(html).toContain("data-pending");
    for (const r of ranges(fieldHtml("x^2", false))) expect(ranges(html)).toContain(r);
    for (const r of ranges(html)) expect(Number(r.split("-")[1])).toBeLessThanOrEqual(3);
  });

  test("an empty slot it fills loses its placeholder, as typing would", () => {
    const plain = ranges(fieldHtml("\\frac{}{}", false));
    const html = pendingHtml(pendingAt("\\frac{}{}", 6), false)!;
    expect(plain).toContain("6-6");
    expect(ranges(html)).toEqual(plain.filter((r) => r !== "6-6"));
  });

  test("nothing without a pending command", () => {
    expect(pendingHtml(MathField.open("a", false), false)).toBeNull();
  });
});
