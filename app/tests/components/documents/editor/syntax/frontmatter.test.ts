import { describe, expect, test } from "bun:test";
import { EditorSelection, EditorState } from "@codemirror/state";
import { ensureSyntaxTree, LanguageSupport } from "@codemirror/language";
import { IterMode, type Tree } from "@lezer/common";
import { noteLanguage } from "@/components/documents/editor/core/language";

/** The tree's nodes in pre-order, nested code pruned. */
function nodes(tree: Tree): string[] {
  const out: string[] = [];
  const c = tree.cursor(IterMode.IgnoreMounts);
  do out.push(`${c.name} ${c.from} ${c.to}`);
  while (c.next());
  return out;
}

/** Types `text` at `at` one character at a time, checking after every
 *  keystroke that CodeMirror's incremental tree equals a fresh parse. */
function typeChecked(doc: string, at: number, text: string): EditorState {
  let state = EditorState.create({ doc, selection: EditorSelection.cursor(at), extensions: new LanguageSupport(noteLanguage) });
  ensureSyntaxTree(state, state.doc.length, 1e9);
  for (const ch of text) {
    state = state.update(state.replaceSelection(ch)).state;
    const tree = ensureSyntaxTree(state, state.doc.length, 1e9)!;
    const text = state.doc.toString();
    expect({ text, nodes: nodes(tree) }).toEqual({ text, nodes: nodes(noteLanguage.parser.parse(text)) });
  }
  return state;
}

const firstNode = (state: EditorState) => ensureSyntaxTree(state, state.doc.length, 1e9)!.topNode.firstChild!.name;

const props = Array.from({ length: 8 }, (_, i) => `key${i}: a value long enough to pass the fragment gap\n`).join("");
const body = "# Heading\n\nA paragraph after the frontmatter, long enough to leave fragments behind it.\n".repeat(4);

describe("frontmatter under incremental parsing", () => {
  test("typing a short block into an empty note", () => {
    expect(firstNode(typeChecked("", 0, "---\n\n---"))).toBe("Frontmatter");
  });

  test("typing a short block above a note", () => {
    expect(firstNode(typeChecked(body, 0, "---\n\n---\n"))).toBe("Frontmatter");
  });

  test("typing a block longer than the fragment gap", () => {
    expect(firstNode(typeChecked("", 0, `---\n${props}---\n${body}`))).toBe("Frontmatter");
  });

  test("closing a long block above a note", () => {
    const doc = `---\n${props}\n${body}`;
    expect(firstNode(typeChecked(doc, 4 + props.length, "---"))).toBe("Frontmatter");
  });

  test("breaking the closing fence turns it back into a rule", () => {
    const doc = `---\n${props}--\n${body}`;
    let state = typeChecked(doc, 4 + props.length + 2, "-");
    expect(firstNode(state)).toBe("Frontmatter");
    state = state.update({ changes: { from: 4 + props.length, to: 4 + props.length + 1 } }).state;
    const tree = ensureSyntaxTree(state, state.doc.length, 1e9)!;
    expect(nodes(tree)).toEqual(nodes(noteLanguage.parser.parse(state.doc.toString())));
    expect(firstNode(state)).toBe("HorizontalRule");
  });

  test("typing below a closed block keeps it", () => {
    const doc = `---\n${props}---\n${body}`;
    expect(firstNode(typeChecked(doc, doc.length, "\nMore text --- here\n---\n"))).toBe("Frontmatter");
  });
});
