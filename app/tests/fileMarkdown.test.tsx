import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import ReactMarkdown from "react-markdown";
import { fileMarkdownPlugins } from "../src/components/files/FileMarkdown";
import { normalizeMath } from "../src/lib/mathMarkdown";

function render(text: string) {
  return renderToStaticMarkup(<ReactMarkdown {...fileMarkdownPlugins(text)}>{normalizeMath(text)}</ReactMarkdown>);
}

describe("library markdown rendering", () => {
  test("math fences work without dollar delimiters", () => {
    for (const source of ["```math\nx^2\n```", "~~~math\nx^2\n~~~"]) {
      expect(render(source)).toContain('class="katex"');
    }
  });
  test("escaped backticks around math are literal text rather than a code span", () => {
    expect(render("\\`$x$\\`")).toContain('class="katex"');
  });
  test("raw HTML math classes keep KaTeX rendering", () => {
    for (const className of ["math-inline", "math-display", "language-math"]) {
      expect(render(`<span class="${className}">x^2</span>`)).toContain('class="katex"');
    }
  });
  test("delimiters, code escaping, raw HTML and plain markdown preserve output", () => {
    expect(render("\\(x^2\\)")).toContain('class="katex"');
    expect(render("`$x$`")).not.toContain('class="katex"');
    expect(render('<strong>raw</strong>')).toContain('<strong>raw</strong>');
    expect(render("**plain** text")).toContain('<strong>plain</strong> text');
  });
});
