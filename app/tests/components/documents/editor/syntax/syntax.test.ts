import { describe, expect, test } from "bun:test";
import { EditorSelection, EditorState } from "@codemirror/state";
import { commonmarkLanguage } from "@codemirror/lang-markdown";
import type { MarkdownParser } from "@lezer/markdown";
import { Language } from "@codemirror/language";
import { MathSyntax } from "@/components/documents/editor/math/mathSyntax";
import { mathAt } from "@/components/documents/editor/math/mathContext";
import { ancestorAt } from "@/components/documents/editor/syntax/syntax";
import { toggleBold } from "@/components/documents/editor/commands";

const language = new Language(commonmarkLanguage.data, (commonmarkLanguage.parser as MarkdownParser).configure(MathSyntax));
const stateFor = (doc: string, from = 0, to = from) => EditorState.create({
  doc, selection: EditorSelection.range(from, to), extensions: [language],
});

describe("editor syntax boundaries", () => {
  test("ancestor lookup keeps caller's boundary bias", () => {
    const state = stateFor("`a`**b**");
    expect(ancestorAt(state, 3, n => n.name === "InlineCode", [-1])?.from).toBe(0);
    expect(ancestorAt(state, 3, n => n.name === "StrongEmphasis", [1])?.from).toBe(3);
  });

  test("bold toggles unwrap inside but start new formatting just outside", () => {
    let state = stateFor("**bold**", 4);
    toggleBold({ state, dispatch: tr => { state = tr.state; } });
    expect(state.doc.toString()).toBe("bold");
    state = stateFor("**bold**", 8);
    toggleBold({ state, dispatch: tr => { state = tr.state; } });
    expect(state.doc.toString()).toBe("**bold******");
    expect(state.selection.main.head).toBe(10);
  });

  test("math content excludes delimiters and empty pairs exclude code", () => {
    const state = stateFor("$x$ and \\(y\\)");
    expect(mathAt(state, 1)).toMatchObject({ from: 1, to: 2, display: false });
    expect(mathAt(state, 0)).toBeNull();
    expect(mathAt(state, 10)).toMatchObject({ from: 10, to: 11 });
    expect(mathAt(stateFor("a $$ b"), 3)).toMatchObject({ from: 3, to: 3 });
    expect(mathAt(stateFor("`$$`"), 2)).toBeNull();
    expect(mathAt(stateFor("$$\nx\n$$"), 3)).toMatchObject({ display: true });
  });
});
