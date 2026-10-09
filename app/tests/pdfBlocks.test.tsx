import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import ReactMarkdown from "react-markdown";
import { fileMarkdownPlugins } from "../src/components/files/FileMarkdown";
import { blocksOnLines, type PagesJson, type PdfBlock } from "../src/lib/citations";
import { normalizeMath } from "../src/lib/mathMarkdown";
import { blockAnchorPlugins, blockRect, buildBlockDoc, refAt } from "../src/lib/pdfBlocks";

const box: PdfBlock["bbox"] = [0.1, 0.1, 0.9, 0.2];

/** A page whose blocks are its "\n\n"-separated parts, in order. */
function page(page_no: number, parts: string[], withBlocks = true) {
  const markdown = parts.join("\n\n");
  const blocks: PdfBlock[] = [];
  let at = 0;
  for (const p of parts) {
    blocks.push({ kind: "text", bbox: box, start: at, end: at + p.length });
    at += p.length + 2;
  }
  return withBlocks ? { page_no, markdown, blocks } : { page_no, markdown };
}

function render(record: PagesJson) {
  const doc = buildBlockDoc(record);
  const html = renderToStaticMarkup(
    <ReactMarkdown {...fileMarkdownPlugins(doc.text, blockAnchorPlugins(doc))}>{doc.text}</ReactMarkdown>,
  );
  return { doc, html };
}

describe("block document", () => {
  test("joins pages as the .md does, math normalised per chunk", () => {
    const record: PagesJson = {
      pages: [page(1, ["# Title", "Inline \\(x^2\\) here", "\\[y\\]"]), page(2, ["Next page"])],
    };
    const md = record.pages.map((p) => p.markdown).join("\n\n");
    const doc = buildBlockDoc(record);
    expect(doc.text).toBe(normalizeMath(md));
    // Every block chunk starts where its text does after normalising.
    expect(refAt(doc, doc.text.indexOf("Inline"))).toEqual({ page: 1, block: 1 });
    expect(refAt(doc, doc.text.indexOf("$$y$$"))).toEqual({ page: 1, block: 2 });
    expect(refAt(doc, doc.text.indexOf("Next page"))).toEqual({ page: 2, block: 0 });
  });

  test("text outside every block stays with its page", () => {
    const markdown = "Lead in\n\nBlocked\n\nTail";
    const start = markdown.indexOf("Blocked");
    const doc = buildBlockDoc({
      pages: [{ page_no: 4, markdown, blocks: [{ kind: "text", bbox: box, start, end: start + 7 }] }],
    });
    expect(doc.text).toBe(markdown);
    expect(refAt(doc, 0)).toEqual({ page: 4 });
    expect(refAt(doc, start)).toEqual({ page: 4, block: 0 });
    expect(refAt(doc, markdown.indexOf("Tail"))).toEqual({ page: 4 });
  });

  test("a record without blocks is one chunk per page", () => {
    const doc = buildBlockDoc({ pages: [page(1, ["a", "b"], false), page(2, ["c"], false)] });
    expect(doc.refs).toEqual([{ page: 1 }, { page: 2 }]);
  });

  test("bad spans are skipped, not trusted", () => {
    const doc = buildBlockDoc({
      pages: [
        {
          page_no: 1,
          markdown: "abc",
          blocks: [
            { kind: "text", bbox: box, start: 2, end: 99 },
            { kind: "text", bbox: box, start: 0, end: 2 },
            { kind: "text", bbox: box, start: 1, end: 3 },
          ],
        },
      ],
    });
    expect(doc.refs).toEqual([{ page: 1, block: 1 }, { page: 1 }]);
  });
});

describe("rendered anchors", () => {
  test("top-level elements carry page and block", () => {
    const { html } = render({ pages: [page(1, ["# Title", "Para one"]), page(2, ["Para two"])] });
    expect(html).toContain('<h1 data-page="1" data-block="0">Title</h1>');
    expect(html).toContain('<p data-page="1" data-block="1">Para one</p>');
    expect(html).toContain('<p data-page="2" data-block="0">Para two</p>');
  });

  test("display maths keeps its tag through KaTeX", () => {
    const { html } = render({ pages: [page(3, ["Before", "$$\nx^2\n$$", "After"])] });
    expect(html).toMatch(/<span class="katex-display" data-page="3" data-block="1">/);
    expect(html).toContain('<p data-page="3" data-block="2">After</p>');
  });

  test("raw HTML tables and list items are tagged", () => {
    const { html } = render({
      pages: [page(2, ["<table><tr><td>1</td></tr></table>", "- one", "- two"])],
    });
    expect(html).toMatch(/<table data-page="2" data-block="0">/);
    expect(html).toMatch(/<li data-page="2" data-block="1">/);
    expect(html).toMatch(/<li data-page="2" data-block="2">/);
  });

  test("output matches the flat render apart from the anchors", () => {
    const record: PagesJson = {
      pages: [page(1, ["## Head", "Some \\(a+b\\) text", "| a | b |\n| - | - |\n| 1 | 2 |"]), page(2, ["![](images/x.png)"])],
    };
    const { html } = render(record);
    const md = normalizeMath(record.pages.map((p) => p.markdown).join("\n\n"));
    const flat = renderToStaticMarkup(<ReactMarkdown {...fileMarkdownPlugins(md)}>{md}</ReactMarkdown>);
    expect(html.replace(/ data-(page|block)="\d+"/g, "")).toBe(flat);
  });
});

describe("geometry and citations", () => {
  test("a box in points, clamped", () => {
    expect(blockRect({ kind: "text", bbox: [0.1, 0.2, 0.5, 1.4], start: 0, end: 1 }, 600, 800)).toEqual({
      x: 60,
      y: 160,
      width: 240,
      height: 640,
    });
    expect(blockRect({ kind: "text", bbox: [0.5, 0.5, 0.5, 0.6], start: 0, end: 1 }, 600, 800)).toBeNull();
  });

  test("cited lines map to the blocks they overlap", () => {
    const p = page(1, ["Line A", "Line B\ncontinued", "Line C"]) as { markdown: string; blocks: PdfBlock[] };
    // Lines: 0 "Line A", 1 "", 2 "Line B", 3 "continued", 4 "", 5 "Line C"
    expect(blocksOnLines(p.markdown, p.blocks, 0, 0)).toEqual([0]);
    expect(blocksOnLines(p.markdown, p.blocks, 3, 3)).toEqual([1]);
    expect(blocksOnLines(p.markdown, p.blocks, 2, 5)).toEqual([1, 2]);
    expect(blocksOnLines(p.markdown, p.blocks, 1, 1)).toEqual([]);
    expect(blocksOnLines(p.markdown, [], 0, 0)).toEqual([]);
  });
});
