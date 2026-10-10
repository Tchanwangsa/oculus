import { describe, expect, test } from "bun:test";
import type { Element, Root } from "hast";
import { renderToStaticMarkup } from "react-dom/server";
import ReactMarkdown from "react-markdown";
import { fileMarkdownPlugins } from "@/components/files/FileMarkdown";
import { mathsReady, rehypeMaths, renderToString } from "@/lib/maths";
import { blockAnchorPlugins } from "@/lib/pdf/blockAnchors";
import { buildBlockDoc } from "@/lib/pdf/pdfBlocks";
import { deep, ready } from "./helpers";

const render = (text: string) =>
  renderToStaticMarkup(<ReactMarkdown {...fileMarkdownPlugins(text)}>{text}</ReactMarkdown>);

/** One maths span in a root, run through the plugin directly. */
function transform(tex: string, className = "math-inline") {
  const tree: Root = {
    type: "root",
    children: [
      { type: "element", tagName: "span", properties: { className: [className] }, children: [{ type: "text", value: tex }] },
    ],
  };
  const messages: { reason: string; options: { ruleId?: string } }[] = [];
  const file = { message: (reason: string, options: { ruleId?: string }) => messages.push({ reason, options }) };
  rehypeMaths()(tree, file as never);
  return { node: tree.children[0] as Element, messages };
}

/** A parsed-PDF record whose blocks are one page's "\n\n"-separated parts. */
function blocksHtml(parts: string[]) {
  const markdown = parts.join("\n\n");
  let at = 0;
  const blocks = parts.map((p) => {
    const block = { kind: "text", bbox: [0.1, 0.1, 0.9, 0.2] as [number, number, number, number], start: at, end: at + p.length };
    at += p.length + 2;
    return block;
  });
  const doc = buildBlockDoc({ pages: [{ page_no: 4, markdown, blocks }] } as never);
  return renderToStaticMarkup(
    <ReactMarkdown {...fileMarkdownPlugins(doc.text, blockAnchorPlugins(doc))}>{doc.text}</ReactMarkdown>,
  );
}

describe("rehypeMaths", () => {
  test("inline, display and a math fence render as KaTeX does", () => {
    const tex = '<annotation encoding="application/x-tex">x^2</annotation>';
    expect(render("$x^2$")).toStartWith('<p><span class="katex"><span class="katex-mathml">');
    expect(render("$x^2$")).toContain(tex);
    expect(render("$$\nx^2\n$$")).toStartWith('<span class="katex-display"><span class="katex">');
    const fence = render("```math\nx^2\n```");
    expect(fence).toStartWith('<span class="katex-display"><span class="katex">');
    // A fence's text keeps its closing newline.
    expect(fence).toContain('<annotation encoding="application/x-tex">x^2\n</annotation>');
    expect(fence).not.toContain("<pre");
  });

  test("a parse error is reported and drawn by the lenient retry", () => {
    const { node, messages } = transform("\\frac{x");
    expect(messages).toHaveLength(1);
    expect(messages[0].options.ruleId).toBe("parseerror");
    expect(node.properties.className).toEqual(["katex-error"]);
    expect(String(node.properties.title)).toStartWith("ParseError: KaTeX parse error: ");
  });

  test("an error the retry throws too becomes a similar span", () => {
    const { node, messages } = transform(deep(320));
    expect(messages).toHaveLength(1);
    expect(node.properties).toMatchObject({ className: ["katex-error"], style: "color:#cc0000" });
    expect(String(node.properties.title)).toStartWith("MathsTrap: The maths engine failed on this formula");
    expect(node.children).toEqual([{ type: "text", value: deep(320) }]);
  });

  test("before the engine is ready a formula is a placeholder, still anchored", async () => {
    await ready();
    expect(() => renderToString(deep(330))).toThrow();
    expect(() => renderToString(deep(331))).toThrow();
    expect(mathsReady()).toBe(false);
    const pending = blocksHtml(["Before", "$$\nx^2\n$$", "After"]);
    expect(pending).toContain('<span class="md-math-pending md-math-pending-display" data-page="4" data-block="1">x^2</span>');
    expect(pending).toContain('<p data-page="4" data-block="2">After</p>');
    expect(transform("x").node.properties.className).toEqual(["md-math-pending"]);
    await ready();
    expect(blocksHtml(["Before", "$$\nx^2\n$$", "After"])).toMatch(/<span class="katex-display" data-page="4" data-block="1">/);
  });
});
