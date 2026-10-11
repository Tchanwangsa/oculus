import { describe, expect, test } from "bun:test";
import { EditorSelection, EditorState, type StateCommand } from "@codemirror/state";
import { markdownLanguage } from "@codemirror/lang-markdown";
import type { MarkdownParser } from "@lezer/markdown";
import { Language, syntaxTree } from "@codemirror/language";
import { MathSyntax } from "@/components/documents/editor/math/mathSyntax";
import {
  toggleBold,
  toggleCode,
  toggleItalic,
  toggleLink,
  toggleStrike,
} from "@/components/documents/editor/commands";

const language = new Language(markdownLanguage.data, (markdownLanguage.parser as MarkdownParser).configure(MathSyntax));

/** Runs `command` with `sel` (a substring of `doc`, or null for a caret at
 *  `at`) selected; returns the document and the text left selected. */
function run(command: StateCommand, doc: string, sel: string | null, at = 0) {
  const from = sel == null ? at : doc.indexOf(sel);
  let state = EditorState.create({
    doc,
    selection: EditorSelection.range(from, sel == null ? from : from + sel.length),
    extensions: [language],
  });
  command({ state, dispatch: (tr) => (state = tr.state) });
  const { from: a, to: b } = state.selection.main;
  return { doc: state.doc.toString(), selected: state.sliceDoc(a, b) };
}

describe("an unwrap takes the mark off the selected text only", () => {
  test("middle of bold splits it in two", () => {
    expect(run(toggleBold, "**foo bar baz**", "bar")).toEqual({ doc: "**foo** bar **baz**", selected: "bar" });
  });

  test("the start or the end of the span leaves a single side", () => {
    expect(run(toggleBold, "**foo bar baz**", "foo").doc).toBe("foo **bar baz**");
    expect(run(toggleBold, "**foo bar baz**", "baz").doc).toBe("**foo bar** baz");
  });

  test("spaces in the selection stay outside the delimiters", () => {
    expect(run(toggleBold, "**foo bar baz**", " bar ").doc).toBe("**foo** bar **baz**");
  });

  test("selecting all the text, or the whole span, removes the mark", () => {
    expect(run(toggleBold, "**foo bar**", "foo bar").doc).toBe("foo bar");
    expect(run(toggleBold, "**foo bar**", "**foo bar**").doc).toBe("foo bar");
  });

  test("a caret still unwraps the whole span", () => {
    expect(run(toggleBold, "**foo bar**", null, 5).doc).toBe("foo bar");
  });

  test("a split never cuts through a nested mark", () => {
    // The italic is wider than the selection, so the bold comes off all of it.
    expect(run(toggleBold, "**a *b c* d**", "c").doc).toBe("**a** *b c* **d**");
    expect(run(toggleBold, "**a `b c` d**", "b").doc).toBe("**a** `b c` **d**");
    expect(run(toggleLink, "[a **b c** d](https://x.dev)", "c").doc).toBe(
      "[a ](https://x.dev)**b c**[ d](https://x.dev)",
    );
  });

  test("italic, strike and code split the same way", () => {
    expect(run(toggleItalic, "*a b c*", "b").doc).toBe("*a* b *c*");
    expect(run(toggleStrike, "~~a b c~~", "b").doc).toBe("~~a~~ b ~~c~~");
    // Code keeps its spaces, so nothing moves outside the backticks.
    expect(run(toggleCode, "`a b c`", "b").doc).toBe("`a `b` c`");
  });

  test("a link splits and both halves keep the destination", () => {
    expect(run(toggleLink, "[foo bar baz](https://x.dev)", "bar")).toEqual({
      doc: "[foo ](https://x.dev)bar[ baz](https://x.dev)",
      selected: "bar",
    });
    expect(run(toggleLink, "[foo bar](https://x.dev)", "foo bar").doc).toBe("foo bar");
  });
});

describe("a mark applied across spans of the same mark merges them", () => {
  /** The syntax-node names around the character at `pos`, innermost first. */
  const around = (doc: string, pos: number) => {
    const names: string[] = [];
    const state = EditorState.create({ doc, extensions: [language] });
    for (let n: ReturnType<typeof syntaxTree>["topNode"] | null = syntaxTree(state).resolveInner(pos, 1); n; n = n.parent) names.push(n.name);
    return names;
  };

  test("italic from inside an italic into plain text, keeping the bold around", () => {
    const { doc, selected } = run(toggleItalic, "**ac*hi*e**ved", "hi*e");
    expect(doc).toBe("**ac*hie***ved");
    expect(selected).toBe("hie");
    const names = around(doc, doc.indexOf("i"));
    expect(names).toContain("Emphasis");
    expect(names).toContain("StrongEmphasis");
    expect(around(doc, doc.indexOf("v"))).not.toContain("Emphasis");
  });

  test("the mirror: from plain text into an italic", () => {
    expect(run(toggleItalic, "*ab* cd", "b* c").doc).toBe("*ab c*d");
    expect(run(toggleItalic, "ab *cd* ef", "b *c").doc).toBe("a*b cd* ef");
  });

  test("spans wholly inside the selection lose their delimiters", () => {
    expect(run(toggleItalic, "a *b* c", "a *b* c").doc).toBe("*a b c*");
    expect(run(toggleBold, "x **a** y **b** z", "a** y **b").doc).toBe("x **a y b** z");
  });

  test("an edge inside another construct widens to its edge", () => {
    expect(run(toggleItalic, "a **b c** d", "c** d").doc).toBe("a ***b c** d*");
  });

  test("a plain selection still just gets the marks, hugging the text", () => {
    expect(run(toggleBold, "one two three", " two ").doc).toBe("one **two** three");
  });
});

describe("whitespace at the ends of a selection is ignored", () => {
  const doc = "tion has **achieved** in this";

  test("a span selected with its surrounding spaces unbolds cleanly", () => {
    for (const sel of [" **achieved** ", " **achieved**", "**achieved** ", "**achieved**"]) {
      expect(run(toggleBold, doc, sel).doc).toBe("tion has achieved in this");
    }
  });

  test("spaces around text inside a span are not what comes off", () => {
    expect(run(toggleBold, "**foo bar baz**", " bar ")).toEqual({ doc: "**foo** bar **baz**", selected: " bar " });
  });

  test("spaces around plain text are not bolded", () => {
    expect(run(toggleBold, "one two three", " two ").doc).toBe("one **two** three");
    expect(run(toggleItalic, "one **two** three", " **two** ").doc).toBe("one ***two*** three");
  });
});
