import { describe, expect, test } from "bun:test";
import type { Element, Root, RootContent } from "hast";
import { renderToStaticMarkup } from "react-dom/server";
import ReactMarkdown from "react-markdown";
import { fileMarkdownPlugins } from "@/components/files/FileMarkdown";
import { BLOCK_MATH_TYPE } from "@/lib/markdown/math";
import { copied } from "@/lib/markdown/mathSelection/clipboard";
import { dropLayer } from "@/lib/markdown/mathSelection/session";
import { MathField, rehypeMaths } from "@/lib/maths";

const render = (text: string) =>
  renderToStaticMarkup(<ReactMarkdown {...fileMarkdownPlugins(text)}>{text}</ReactMarkdown>);

const unescape = (s: string) =>
  s.replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#x27;/g, "'").replace(/&amp;/g, "&");

/** Each formula's annotation text and the ranges of its single-letter glyphs. */
function formulas(html: string) {
  return html.split('<span class="katex">').slice(1).map((part) => {
    const tex = unescape(/<annotation encoding="application\/x-tex">([\s\S]*?)<\/annotation>/.exec(part)![1]);
    const glyphs = [...part.matchAll(/data-s="(\d+)" data-e="(\d+)"[^>]*>([a-z0-9])<\/span>/g)].map((m) => ({
      from: Number(m[1]),
      to: Number(m[2]),
      glyph: m[3],
    }));
    return { tex, glyphs };
  });
}

const md = [
  "Inline $\\frac{a+x}{b}$ and $ y_1 < z^2 $ here.",
  "$$\n\\begin{pmatrix}1&2\\\\3&4\\end{pmatrix} + \\frac{p}{q}\n$$",
  "```math\n  a \\\\ b\n```",
].join("\n\n");

describe("rehypeMaths", () => {
  test("draws the source map and is still KaTeX's markup", () => {
    const html = render(md);
    expect(html).toContain('<span class="katex">');
    expect(html).toContain('<span class="katex-display">');
    expect(html).toContain("data-s=");
  });

  test("the annotation is the string the offsets index, untrimmed", () => {
    const found = formulas(render(md));
    expect(found.map((f) => f.tex)).toEqual([
      "\\frac{a+x}{b}",
      "y_1 < z^2",
      "\\begin{pmatrix}1&2\\\\3&4\\end{pmatrix} + \\frac{p}{q}",
      "  a \\\\ b\n",
    ]);
    for (const { tex, glyphs } of found) {
      expect(glyphs.length).toBeGreaterThan(0);
      for (const g of glyphs) expect(tex.slice(g.from, g.to)).toBe(g.glyph);
      // The field opens on it, so a press can select in it.
      expect(() => MathField.open(tex, false).free()).not.toThrow();
    }
  });

  test("line ends are made \\n before rendering, as the annotation reads", () => {
    const tree: Root = {
      type: "root",
      children: [{ type: "element", tagName: "span", properties: { className: ["math-display"] }, children: [{ type: "text", value: "a \\\\\r\nb" }] }],
    };
    rehypeMaths()(tree, { message() {} } as never);
    let tex = "";
    const letters: string[] = [];
    const walk = (n: Root | RootContent) => {
      if (n.type !== "element" && n.type !== "root") return;
      const el = n as Element;
      if (el.tagName === "annotation") tex = (el.children[0] as { value: string }).value;
      const only = el.children?.length === 1 && el.children[0].type === "text" ? el.children[0].value : "";
      if (el.properties?.dataS != null && /^[a-z]$/.test(only)) letters.push(`${el.properties.dataS}-${el.properties.dataE}`);
      for (const c of n.children) walk(c);
    };
    walk(tree);
    expect(tex).toBe("a \\\\\nb");
    expect(letters.map((r) => tex.slice(...(r.split("-").map(Number) as [number, number])))).toEqual(["a", "b"]);
  });
});

describe("copied", () => {
  test("an inline selection copies its TeX, trimmed, as text and LaTeX", () => {
    expect(copied("\\frac{a+x}{b}", [6, 8], false)).toEqual([
      ["text/plain", "a+"],
      ["application/x-latex", "a+"],
    ]);
  });

  test("a display selection also copies its $$ lines, one row per line", () => {
    const tex = "\na \\\\ b \\\\ c\n";
    const out = copied(tex, [0, tex.length], true);
    expect(out[0]).toEqual(["text/plain", "a \\\\ b \\\\ c"]);
    expect(out[2]).toEqual([BLOCK_MATH_TYPE, "$$\na \\\\\nb \\\\\nc\n$$"]);
  });

  test("an empty or blank range copies nothing", () => {
    expect(copied("x + y", [1, 1], false)).toEqual([]);
    expect(copied("x + y", [1, 2], false)).toEqual([]);
  });

  test("the model widens a drag into a numerator over the whole fraction", () => {
    const tex = "y=\\frac{a+x}{b}+1";
    const f = MathField.open(tex, false);
    const stop = (offset: number, slot: number) =>
      [...f.stops].findIndex((o, id) => o === offset && f.stopSlots[id] === slot);
    const sel = f.select(stop(2, 0), stop(9, 1));
    expect(copied(tex, sel.selected, false)[0]).toEqual(["text/plain", "\\frac{a+x}{b}"]);
  });
});

describe("closing a selection", () => {
  /** A band layer and its formula, recording what close does to them. */
  function fakes(reopened = false) {
    const log: string[] = [];
    const layer = { replaceChildren: () => log.push("bands cleared"), remove: () => log.push("layer removed") };
    const root = {
      querySelector: () => (reopened ? {} : null),
      removeAttribute: (name: string) => log.push(`${name} removed`),
    };
    const frames: (() => void)[] = [];
    const frame = (run: () => void) => frames.push(run);
    const tick = () => frames.splice(0).forEach((run) => run());
    return { log, layer, root, frame, tick };
  }

  test("clears the bands at once and drops the layer two frames later", () => {
    const f = fakes();
    dropLayer(f.layer, f.root as never, f.frame);
    expect(f.log).toEqual(["bands cleared"]);
    f.tick();
    expect(f.log).toEqual(["bands cleared"]);
    f.tick();
    expect(f.log).toEqual(["bands cleared", "layer removed", "data-math-selecting removed"]);
  });

  test("a selection reopened on the formula meanwhile keeps its stacking", () => {
    const f = fakes(true);
    dropLayer(f.layer, f.root as never, f.frame);
    f.tick();
    f.tick();
    expect(f.log).toEqual(["bands cleared", "layer removed"]);
  });
});
