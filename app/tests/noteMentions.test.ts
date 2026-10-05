import { describe, expect, test } from "bun:test";
import { inlineCodeCitation, mentionText } from "../src/components/documents/editor/mentionSyntax";

describe("note mentions", () => {
  test("@ writes the backticked library path the chat sends", () => {
    expect(mentionText("courses/COMP30027_2026_SM2/files/week-3.pdf")).toBe("`courses/COMP30027_2026_SM2/files/week-3.pdf`");
  });

  test("what @ writes reads back as a chip", () => {
    const path = "courses/COMP30027_2026_SM2/files/week-3.pdf";
    expect(inlineCodeCitation(mentionText(path))).toBe(path);
  });

  test("a span holding only a citation is a chip", () => {
    expect(inlineCodeCitation("`courses/X/files/a.pdf#page=3`")).toBe("courses/X/files/a.pdf#page=3");
    expect(inlineCodeCitation("`agents/attachments/x.png`")).toBe("agents/attachments/x.png");
    expect(inlineCodeCitation("`` courses/X/files/a.pdf ``")).toBe("courses/X/files/a.pdf");
  });

  test("ordinary code, commands and broken spans stay code", () => {
    expect(inlineCodeCitation("`foo()`")).toBeNull();
    expect(inlineCodeCitation("`oculus read courses/x.pdf`")).toBeNull();
    expect(inlineCodeCitation("``")).toBeNull();
    expect(inlineCodeCitation("`courses/X/files/a.pdf")).toBeNull();
    expect(inlineCodeCitation("`courses/X/\nfiles/a.pdf`")).toBeNull();
  });
});
