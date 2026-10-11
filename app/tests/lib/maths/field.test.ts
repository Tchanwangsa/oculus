import { describe, expect, test } from "bun:test";
import { MathField, MathsTrap, fieldShortcuts, renderToString, type FieldCommand, type Step } from "@/lib/maths";
import { deep, ready } from "./helpers";

/** `source` with the step's changes, then its rewrite, applied as the edit
 *  model's own tests apply them: each list in reverse order. */
function applied(source: string, step: Step): string {
  for (const changes of [step.changes, step.rewrite ?? []]) {
    for (const { from, to, insert } of [...changes].reverse()) {
      source = source.slice(0, from) + insert + source.slice(to);
    }
  }
  return source;
}

/** Runs each command in turn, checking every step's changes. */
function press(field: MathField, ...commands: FieldCommand[]): Step {
  let step: Step | null = null;
  for (const command of commands) {
    step = field.run(command);
    expect(applied(field.source, step)).toBe(step.field.source);
    field = step.field;
  }
  return step!;
}

const type = (text: string): FieldCommand[] => [...text].map((c) => ({ insert: c }));

describe("maths field", () => {
  test("opens with the caret at the end", () => {
    const field = MathField.open("x^2", false);
    expect(field.source).toBe("x^2");
    expect(field.display).toBe(false);
    expect(field.mode).toBe("math");
    expect(field.pending).toBeUndefined();
    expect(field.head).toBe(field.stops.length - 1);
    expect(field.selected).toEqual([3, 3]);
    expect(field.stopSlots.length).toBe(field.stops.length);
    expect(field.slots.map((s) => s.kind)).toEqual(["row", "sup"]);
    expect(field.slots[1]).toMatchObject({ bounds: "bare", from: 2, to: 3, parent: 0 });
  });

  test("unparseable source throws KaTeX's ParseError", () => {
    expect(() => MathField.open("\\frac{x", false)).toThrow(expect.objectContaining({ name: "ParseError" }));
  });

  test("typing changes the source as the steps say", () => {
    const step = press(MathField.open("", false), ...type("x^23"));
    expect(step.field.source).toBe("x^{23}");
    expect(step.isolate).toBe(false);
    expect(step.effect).toBeUndefined();
  });

  test("a shortcut is a rewrite after its key, and Esc puts the keys back", () => {
    const sin = press(MathField.open("", false), ...type("sin"));
    expect(sin.changes).toEqual([{ from: 2, to: 2, insert: "n" }]);
    expect(sin.rewrite).toEqual([{ from: 0, to: 0, insert: "\\" }]);
    expect(sin.field.source).toBe("\\sin");
    const back = press(sin.field, "escape");
    expect(back.field.source).toBe("sin");
    expect(back.isolate).toBe(true);
  });

  test("offsets in a Thai text run are the string's indices", () => {
    const source = "\\text{สวัสดี}";
    const field = MathField.open(source, false);
    // No stop between a consonant and its vowel mark (สว|ั).
    expect([...field.stops]).toEqual([0, 6, 7, 9, 10, 12, 13]);
    const text = field.slots[1];
    expect(text).toMatchObject({ kind: "text", text: true });
    expect(source.slice(text.from, text.to)).toBe("สวัสดี");
    const before = field.caretAt(source.indexOf("ด"));
    expect(before.selected).toEqual([10, 10]);
    expect(before.mode).toBe("text");
    const step = press(before, { insert: "ก" });
    expect(step.changes).toEqual([{ from: 10, to: 10, insert: "ก" }]);
    expect(step.field.source).toBe("\\text{สวัสกดี}");
  });

  test("an offset inside a surrogate pair is refused", () => {
    const field = MathField.open("\\text{𝒜}", false);
    expect([...field.stops]).toEqual([0, 6, 8, 9]);
    expect(() => field.caretAt(7)).toThrow();
    expect(field.caretAt(8).selected).toEqual([8, 8]);
  });

  test("↑ reads each stop's x by stop id, NaN unmeasured", () => {
    const field = MathField.open("\\frac{ab}{c}", false).caretAt(10);
    const up = press(field, { up: [NaN, 0, 5, 10, 9, 14, NaN] });
    expect(up.field.selected).toEqual([8, 8]);
    expect(press(field, { down: [] }).effect).toBe("leaveDown");
  });

  test("selections widen, and a pending command is the view's", () => {
    const field = MathField.open("a+\\frac{b}{c}", false);
    // From before `a` to inside the numerator takes the whole fraction.
    const widened = field.select(0, 3);
    expect(widened.selected).toEqual([0, 13]);
    const command = press(field, { insert: "\\" }).field;
    expect(command.mode).toBe("command");
    expect(command.pending).toBe("");
    expect(command.withPending(undefined).mode).toBe("math");
  });

  test("the shortcut table is the model's", () => {
    expect(fieldShortcuts()).toContainEqual(["sin", "\\sin"]);
  });

  test("a trap elsewhere leaves a field usable", async () => {
    await ready();
    const field = press(MathField.open("", false), ...type("xy")).field;
    expect(() => renderToString(deep(330))).toThrow(MathsTrap);
    await ready();
    const step = press(field, ...type("z"));
    expect(step.field.source).toBe("xyz");
  });

  test("a trap in a step throws a MathsTrap and the field goes on", async () => {
    await ready();
    const field = MathField.open("x", false);
    expect(() => field.run({ paste: deep(331) })).toThrow(MathsTrap);
    await ready();
    expect(press(field, { insert: "y" }).field.source).toBe("xy");
  });

  test("collecting fields of a trapped instance never calls into it", async () => {
    await ready();
    let fields: MathField[] = Array.from({ length: 50 }, () => MathField.open("x^2", false));
    expect(() => renderToString(deep(332))).toThrow(MathsTrap);
    fields = [];
    expect(fields).toEqual([]);
    // A call into the trapped instance would trap here, failing the test.
    for (let i = 0; i < 3; i++) {
      Bun.gc(true);
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
  });

  test("a freed field opens again on its next step", () => {
    const field = MathField.open("a+b", false).caretAt(1);
    field.free();
    field.free();
    const step = press(field, { insert: "c" });
    expect(step.field.source).toBe("ac+b");
  });
});
