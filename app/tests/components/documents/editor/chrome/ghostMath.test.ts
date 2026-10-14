import { describe, expect, test } from "bun:test";
import { ghostParts } from "@/components/documents/editor/chrome/ghostMath";

describe("maths in AI suggestion previews", () => {
  test("renders the totient examples while preserving the insertion text", () => {
    const text = "- E.g. $\\phi(1)=1$, $\\phi(6)=2$, $\\phi(7)=6$; if $p$ is prime then $\\phi(p)=p-1$.";
    const parts = ghostParts(text, true);
    expect(parts.map((part) => part.text).join("")).toBe(text);
    expect(parts.filter((part) => part.html).length).toBe(5);
    expect(parts.filter((part) => part.html).every((part) => part.html!.includes('class="katex"'))).toBe(true);
  });

  test("recognises both inline and display delimiters", () => {
    const text = "For \\(x^2\\):\n$$\n\\frac{1}{2}\n$$\n\\[y^2\\]";
    const parts = ghostParts(text, true);
    expect(parts.filter((part) => part.html).map((part) => part.display)).toEqual([false, true, true]);
    expect(parts.map((part) => part.text).join("")).toBe(text);
    expect(parts.filter((part) => part.display).every((part) => part.html!.includes("katex-display"))).toBe(true);
  });

  test("leaves code, escaped dollars, currency and incomplete maths as source", () => {
    for (const text of ["`$x$`", "```tex\n$x$\n```", "\\$x\\$", "$5 and $10", "$x", "$$\nx^2"]) {
      expect(ghostParts(text, true)).toEqual([{ text }]);
    }
  });

  test("a broken equation doesn't stop later valid maths rendering", () => {
    const text = "$\\notARealCommand{x}$ then $x^2$";
    const parts = ghostParts(text, true);
    expect(parts[0]).toEqual({ text: "$\\notARealCommand{x}$ then " });
    expect(parts[1].html).toContain('class="katex"');
    expect(parts.map((part) => part.text).join("")).toBe(text);
  });

  test("Raw mode keeps LaTeX and prose verbatim", () => {
    const text = "<b>Example</b> $x$\n$$y$$";
    expect(ghostParts(text, false)).toEqual([{ text }]);
    expect(ghostParts("<b>Example</b> $x$", true)[0]).toEqual({ text: "<b>Example</b> " });
  });
});
