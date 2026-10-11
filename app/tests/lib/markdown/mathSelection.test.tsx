import { describe, expect, test } from "bun:test";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import type { Element, Root, RootContent } from "hast";
import { renderToStaticMarkup } from "react-dom/server";
import ReactMarkdown from "react-markdown";
import { fileMarkdownPlugins } from "@/components/files/FileMarkdown";
import { BLOCK_MATH_TYPE } from "@/lib/markdown/math";
import { copied, copiesAsBlock } from "@/lib/markdown/mathSelection/clipboard";
import { formulaMarkdown } from "@/lib/markdown/selection";
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

/** The user's formula: a fraction, then a two-row `cases`. */
const CASES = "\\frac{1}{\\sqrt2}(|0\\rangle+(-1)^{s_i}|1\\rangle) = \\begin{cases}|+\\rangle & s_i = 0 \\text{ (no CNOT)}\\\\ |-\\rangle & s_i=1\\end{cases}";

/** `tex`'s model with the selection dragged from the stop at offset
 *  `from` to the one at `to` (the first stop there). */
function dragged(tex: string, display: boolean, from: number, to: number): MathField {
  const f = MathField.open(tex, display);
  const stop = (offset: number) => [...f.stops].indexOf(offset);
  return f.select(stop(from), stop(to));
}

describe("copied", () => {
  test("part of one row copies as inline $…$, never as a block", () => {
    const from = CASES.indexOf("s_i = 0");
    const sel = dragged(CASES, true, from, CASES.indexOf("\\\\ |-"));
    expect(copied(sel, true)).toEqual([
      ["text/plain", "$s_i = 0 \\text{ (no CNOT)}$"],
      ["application/x-latex", "s_i = 0 \\text{ (no CNOT)}"],
    ]);
  });

  test("a whole inline formula on one row copies as $…$", () => {
    const tex = "\\frac{a+x}{b}";
    expect(copied(dragged(tex, false, 0, tex.length), false)[0]).toEqual(["text/plain", "$\\frac{a+x}{b}$"]);
  });

  test("a drag across an environment's rows takes it whole, as a block", () => {
    const sel = dragged(CASES, true, CASES.indexOf("0 \\text"), CASES.indexOf("=1"));
    const env = CASES.slice(CASES.indexOf("\\begin{cases}"));
    expect(CASES.slice(...sel.selected).trim()).toBe(env);
    const block = "$$\n\\begin{cases}\n|+\\rangle & s_i = 0 \\text{ (no CNOT)} \\\\\n|-\\rangle & s_i=1\n\\end{cases}\n$$";
    expect(copied(sel, true)).toEqual([
      ["text/plain", block],
      ["application/x-latex", env],
      [BLOCK_MATH_TYPE, block],
    ]);
  });

  test("a whole display formula copies as $$ lines, one row each", () => {
    const tex = "\na \\\\ b \\\\ c\n";
    const f = MathField.open(tex, true).run("selectAll").field;
    const out = copied(f, true);
    expect(out[0]).toEqual(["text/plain", "$$\na \\\\\nb \\\\\nc\n$$"]);
    expect(out[2]).toEqual([BLOCK_MATH_TYPE, "$$\na \\\\\nb \\\\\nc\n$$"]);
    expect(copiesAsBlock(MathField.open("x^2", true).run("selectAll").field, true)).toBe(true);
  });

  test("two of a display's top-level rows copy as a block", () => {
    const tex = "a \\\\ b \\\\ c";
    expect(copiesAsBlock(dragged(tex, true, 0, tex.indexOf("b") + 1), true)).toBe(true);
    expect(copiesAsBlock(dragged(tex, true, tex.indexOf("b"), tex.indexOf("b") + 1), true)).toBe(false);
  });

  test("an empty or blank range copies nothing", () => {
    expect(copied({ source: "x + y", selected: [1, 1], slots: [] }, false)).toEqual([]);
    expect(copied({ source: "x + y", selected: [1, 2], slots: [] }, false)).toEqual([]);
  });

  test("the model widens a drag into a numerator over the whole fraction", () => {
    const tex = "y=\\frac{a+x}{b}+1";
    const f = MathField.open(tex, false);
    const stop = (offset: number, slot: number) =>
      [...f.stops].findIndex((o, id) => o === offset && f.stopSlots[id] === slot);
    const sel = f.select(stop(2, 0), stop(9, 1));
    expect(copied(sel, false)[0]).toEqual(["text/plain", "$\\frac{a+x}{b}$"]);
  });

  test("a text selection writes each formula it takes whole by its shape", () => {
    expect(formulaMarkdown("x^2", false)).toBe("$x^2$");
    expect(formulaMarkdown("a \\\\ b", true)).toBe("$$\na \\\\\nb\n$$");
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

describe("a selected formula", () => {
  test("gets bands, never a box: no whole-formula class or attribute is drawn or styled", () => {
    const src = join(import.meta.dir, "../../../src");
    const files = (dir: string): string[] =>
      readdirSync(dir).flatMap((name) => {
        const path = join(dir, name);
        return statSync(path).isDirectory() ? files(path) : /\.(tsx?|css)$/.test(name) ? [path] : [];
      });
    const boxed = files(src).filter((f) => /data-math-selected|cm-math-selected/.test(readFileSync(f, "utf8")));
    expect(boxed).toEqual([]);
  });
});
