import { describe, expect, test } from "bun:test";

import { DEFAULT_INDENT, detectIndent } from "@/components/code/indent";

describe("indent detection", () => {
  test("a file without indented lines takes four spaces", () => {
    expect(detectIndent("")).toBe(DEFAULT_INDENT);
    expect(detectIndent("x = 1\nprint(x)\n")).toBe("    ");
  });

  test("the commonest step between a line and a deeper next one", () => {
    expect(detectIndent("def f():\n  if x:\n    return 1\n  return 2\n")).toBe("  ");
    expect(detectIndent("class A:\n    def f(self):\n        pass\n")).toBe("    ");
    expect(detectIndent("{\n   a\n      b\n}\n")).toBe("   ");
  });

  test("blank lines are skipped, and a step is measured across them", () => {
    expect(detectIndent("def f():\n\n  return 1\n\n\ndef g():\n\n  return 2\n")).toBe("  ");
  });

  test("a docstring's continuation lines do not outvote the code's steps", () => {
    const text = "def f():\n    \"\"\"Doc\n    more\n    \"\"\"\n    if x:\n        y()\n    for a in b:\n        c()\n";
    expect(detectIndent(text)).toBe("    ");
  });

  test("tabs win when most indented lines start with one", () => {
    expect(detectIndent("int main() {\n\tif (x) {\n\t\treturn 1;\n\t}\n}\n")).toBe("\t");
  });

  test("a tie between steps goes to four, then the narrower", () => {
    expect(detectIndent("a:\n  b\nc:\n    d\n")).toBe("    ");
    expect(detectIndent("a:\n  b\nc:\n   d\n")).toBe("  ");
  });
});
