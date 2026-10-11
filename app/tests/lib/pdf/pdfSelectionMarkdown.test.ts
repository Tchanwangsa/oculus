import { describe, expect, test } from "bun:test";
import {
  align,
  mapBoundary,
  markdownOfSelection,
  markdownSkeleton,
  parsedDoc,
  plainText,
  sliceMarkdown,
  textSkeleton,
  type PageSelection,
} from "@/lib/pdf/pdfSelectionMarkdown";

/** The selection from the first `from` to the end of the first `to` after it,
 *  on one page's text layer. */
function on(page: number, layer: string, from: string, to: string): PageSelection {
  const start = layer.indexOf(from);
  const end = layer.indexOf(to, start) + to.length;
  if (start < 0 || end < to.length) throw new Error(`not in layer: ${from} / ${to}`);
  return { page, layer, start, end };
}

const img = (src: string) => `/abs/${src}`;

describe("markdown skeleton", () => {
  test("syntax adds nothing; alt text and maths letters do", () => {
    const md = "## Title with $\\alpha + x$ and ![Fig 1: cat](a_images/x.png) see [link](http://u)";
    const sk = markdownSkeleton(md);
    expect(sk.text).toBe("titlewithαxandfig1catseelink");
    expect(sk.atoms.map((a) => md.slice(a.start, a.end))).toEqual([
      "$\\alpha + x$",
      "![Fig 1: cat](a_images/x.png)",
      "[link](http://u)",
    ]);
    const image = sk.atoms[1];
    expect(md.slice(image.url!.start, image.url!.end)).toBe("a_images/x.png");
    // Every char points back at where it came from.
    expect(md.slice(sk.from[0], sk.to[0])).toBe("T");
    expect(md.slice(sk.from[9], sk.to[9])).toBe("\\alpha");
  });

  test("TeX: command names, environment names and column specs are dropped", () => {
    expect(markdownSkeleton("$$\n\\begin{array} { l l } a & b \\\\ \\ell \\end{array}\n$$").text).toBe("abl");
    expect(markdownSkeleton("$\\mathsf { X } , \\frac{\\sqrt{3}}{2}$").text).toBe("x32");
    // Math-italic codepoints in a text layer fold to the same letters.
    expect(textSkeleton("𝑋 = 𝛼𝑥").text).toBe("xαx");
  });

  test("an HTML table is one atom whose cells count", () => {
    const md = "Before\n\n<table><tr><td>a1</td><td>b &amp; c</td></tr></table>\n\nAfter";
    const sk = markdownSkeleton(md);
    expect(sk.text).toBe("beforea1bcafter");
    expect(sk.atoms).toHaveLength(1);
    expect(md.slice(sk.atoms[0].start, sk.atoms[0].end)).toStartWith("<table>");
    expect(md.slice(sk.atoms[0].start, sk.atoms[0].end)).toEndWith("</table>");
  });

  test("escapes are literal; emphasis pairs follow CommonMark flanking", () => {
    expect(markdownSkeleton("costs \\$5 or \\$6 \\*not\\* emphasis").atoms).toHaveLength(0);
    expect(markdownSkeleton("costs \\$5 or \\$6 \\*not\\* emphasis").pairs).toHaveLength(0);
    expect(markdownSkeleton("snake_case_name and 2 * 3 * 4").pairs).toHaveLength(0);
    const md = "**bold** and *it* and ***both***";
    const pairs = markdownSkeleton(md).pairs.map((p) => md.slice(p.open.start, p.open.end));
    expect(pairs.sort()).toEqual(["*", "*", "**", "**"]);
    // A paragraph break ends a run's chance to pair.
    expect(markdownSkeleton("*open\n\nclose*").pairs).toHaveLength(0);
  });
});

describe("alignment", () => {
  const md =
    "## Recap: the X gate\n\nThe X gate flips the qubit: $X = \\left[ \\begin{array} { l l } 0 & 1 \\\\ 1 & 0 \\end{array} \\right]$ so the state changes.\n\nA second paragraph about the Bloch sphere and its axes.";
  const layer =
    "MULT20015 Elements of Quantum Computing\nRecap: the X gate\nThe X gate flips the qubit: X =\n\uf8ff 0 1\n1 0\n\x00 so the state changes.\nA second paragraph about the Bloch sphere and its axes.";

  test("anchors through garbled maths and an unmatched running header", () => {
    const t = textSkeleton(layer).text;
    const m = markdownSkeleton(md).text;
    const al = align(t, m);
    expect(al).not.toBeNull();
    expect(al!.coverage).toBeGreaterThan(0.6);
    const at = t.indexOf("sothestate");
    expect(m.slice(mapBoundary(al!, at), mapBoundary(al!, at) + 10)).toBe("sothestate");
  });

  test("reordered text keeps the longer order", () => {
    const m = textSkeleton("alpha paragraph comes first here. beta paragraph follows it there. gamma closes.").text;
    const t = textSkeleton("beta paragraph follows it there. alpha paragraph comes first here. gamma closes.").text;
    const al = align(t, m);
    expect(al).not.toBeNull();
    // Runs stay increasing in both strings.
    for (let i = 1; i < al!.runs.length; i++) {
      expect(al!.runs[i].t).toBeGreaterThanOrEqual(al!.runs[i - 1].t + al!.runs[i - 1].len);
      expect(al!.runs[i].m).toBeGreaterThanOrEqual(al!.runs[i - 1].m + al!.runs[i - 1].len);
    }
  });

  test("unrelated text is not trusted", () => {
    const t = textSkeleton("Completely different words about networking packets and routers").text;
    const m = markdownSkeleton("## Privacy\n\nConsent models in data gathering, explained slowly").text;
    expect(align(t, m)).toBeNull();
  });

  test("a tiny text layer matches whole or not at all", () => {
    expect(align("lecture4", "mult20015lecture4")?.runs).toEqual([{ t: 0, m: 9, len: 8 }]);
    expect(align("abc", "abcxabc")).toBeNull();
  });
});

describe("slicing", () => {
  test("snaps out to whole maths, delimiters and block prefixes", () => {
    const md = "## The **really important** idea\n\n- uses $e^{i\\pi} + 1 = 0$ often";
    const sk = markdownSkeleton(md);
    // From "really" (just inside `**`) to "imp" (inside the bold).
    const s = md.indexOf("really");
    expect(sliceMarkdown(md, sk, s, md.indexOf("ortant"))).toBe("**really imp**");
    // From "The": only `## ` precedes, so the heading marker comes along.
    expect(sliceMarkdown(md, sk, md.indexOf("The"), md.indexOf(" idea"))).toBe("## The **really important**");
    // Into the maths: the whole span.
    expect(sliceMarkdown(md, sk, md.indexOf("uses"), md.indexOf("pi"))).toBe("- uses $e^{i\\pi} + 1 = 0$");
  });

  test("images go through resolveImage; links and tables come whole", () => {
    const md = "See [the docs](https://x.y/z) and ![Figure 2](lec_images/f.png)\n\n<table><tr><td>1</td></tr></table> end";
    const sk = markdownSkeleton(md);
    expect(sliceMarkdown(md, sk, md.indexOf("docs"), md.indexOf("Figure") + 3, img)).toBe(
      "[the docs](https://x.y/z) and ![Figure 2](/abs/lec_images/f.png)",
    );
    expect(sliceMarkdown(md, sk, md.indexOf("<td>1") + 4, md.length)).toBe("<table><tr><td>1</td></tr></table> end");
  });
});

describe("selection", () => {
  const pages = new Map<number, string>([
    [
      1,
      "## Applying a gate to a state\n\nWe apply $\\widehat { U }$ to the state and **measure the outcome** afterwards.\n\n![Figure 1: the Bloch sphere](lec_images/bloch.png)",
    ],
    [2, "## Second page\n\nNothing but words on this page, a plain middle page of text."],
    [3, "## Third page\n\nThe final page opens here and the selection ends somewhere in it."],
  ]);
  const doc = parsedDoc(pages);
  const layer1 =
    "Applying a gate to a state\nWe apply U\nb to the state and measure the outcome afterwards.\nFigure 1: the Bloch sphere";
  const layer3 = "Third page\nThe final page opens here and the selection ends somewhere in it.";

  test("within a page: maths whole, emphasis balanced", () => {
    expect(markdownOfSelection(doc, [on(1, layer1, "apply U", "measure the")], img)).toBe(
      "apply $\\widehat { U }$ to the state and **measure the**",
    );
  });

  test("running to the page's end brings its trailing figure", () => {
    const sel = { page: 1, layer: layer1, start: layer1.indexOf("measure"), end: layer1.length };
    expect(markdownOfSelection(doc, [sel], img)).toBe(
      "**measure the outcome** afterwards.\n\n![Figure 1: the Bloch sphere](/abs/lec_images/bloch.png)",
    );
    // Ending inside the paragraph leaves it out.
    expect(markdownOfSelection(doc, [on(1, layer1, "measure", "afterwards")], img)).toBe(
      "**measure the outcome** afterwards",
    );
  });

  test("across pages: covered pages whole, rendered or not", () => {
    const sel: PageSelection[] = [
      on(1, layer1, "outcome", "afterwards"),
      { page: 2, layer: null, start: null, end: null },
      { ...on(3, layer3, "Third", "opens here"), start: 0 },
    ];
    sel[0].end = layer1.length;
    expect(markdownOfSelection(doc, sel, img)).toBe(
      [
        "**outcome** afterwards.",
        "![Figure 1: the Bloch sphere](/abs/lec_images/bloch.png)",
        "## Second page",
        "Nothing but words on this page, a plain middle page of text.",
        "## Third page",
        "The final page opens here",
      ].join("\n\n"),
    );
  });

  test("an end that cannot be aligned copies that page's text", () => {
    const garbled = "zzqx vvkp wwmr ttyl nnbh qqzx kkvp";
    const sel: PageSelection[] = [
      { page: 2, layer: garbled, start: 5, end: null },
      { ...on(3, layer3, "Third", "opens here"), start: 0 },
    ];
    expect(markdownOfSelection(doc, sel)).toBe("vvkp wwmr ttyl nnbh qqzx kkvp\n\n## Third page\n\nThe final page opens here");
    // Nothing usable: leave the browser's copy alone.
    expect(markdownOfSelection(doc, [{ page: 2, layer: garbled, start: 5, end: 20 }])).toBe("");
  });

  test("a page with no markdown copies as text, `$` escaped", () => {
    const sel: PageSelection[] = [
      { page: 3, layer: layer3, start: layer3.indexOf("selection"), end: null },
      { page: 4, layer: "Costs $5 and $6 here", start: null, end: 8 },
    ];
    expect(markdownOfSelection(doc, sel)).toBe("selection ends somewhere in it.\n\nCosts \\$5");
    expect(plainText("a\x00b\ufb01c $x$")).toBe("abfic \\$x\\$");
  });

  test("text MinerU filed under the previous page is found there", () => {
    const paper = parsedDoc(
      new Map([
        [1, "## Method\n\nThe study ran for three years and the students were asked to maintain the existing code base carefully.\n\n1 of 2"],
        [2, "## Results\n\nMost teams improved the software."],
      ]),
    );
    const p1 = "Journal 1 of 2\nMethod\nThe study ran for three years and the students 12\n";
    const p2 = "Journal 2 of 2\nwere asked to maintain the existing code base carefully. 13\nResults\nMost teams improved the software. 14";
    // Only the carried part, on page 2.
    expect(markdownOfSelection(paper, [on(2, p2, "maintain", "code base")])).toBe("maintain the existing code base");
    // Across the break, once and without the margin numbers.
    expect(
      markdownOfSelection(paper, [
        { ...on(1, p1, "three years", "students"), end: p1.length },
        on(2, p2, "were", "Most teams"),
      ]),
    ).toBe(
      "three years and the students were asked to maintain the existing code base carefully.\n\n1 of 2\n\n## Results\n\nMost teams",
    );
  });
});
