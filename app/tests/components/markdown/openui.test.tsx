import { afterAll, describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import ReactMarkdown from "react-markdown";

// The widgets render text through `InlineMd`, whose imports reach modules
// that listen on `window` and `document` at load; tests run without a DOM.
const globals = globalThis as { window?: unknown; document?: unknown };
const stubbed = (["window", "document"] as const).filter((k) => !(k in globals));
for (const k of stubbed) globals[k] = Object.assign(new EventTarget(), { documentElement: { style: {} } });
afterAll(() => {
  for (const k of stubbed) delete globals[k];
});
const { parseProgram } = await import("@/components/markdown/openui/parse");
const { PROMPT_EXAMPLES } = await import("@/components/markdown/openui/prompt");
const { repairTex } = await import("@/components/markdown/openui/components/Frame");
const { MD_COMPONENTS } = await import("@/components/markdown/MdComponents");
const { remarkWidgets } = await import("@/components/markdown/openui/remarkWidgets");
const { Chart } = await import("@/components/markdown/openui/components/Chart");

const PROGRAMS = {
  flashcards: `root = Stack([deck])
deck = Flashcards("Krebs cycle", [Card("Where does it run?", "The mitochondrial matrix"), Card("Net NADH per turn?", "3 NADH")])`,
  quiz: `root = Stack([quiz])
quiz = Quiz("Big-O", [q1])
q1 = Question("Binary search on $n$ items?", [Option("$O(n)$"), Option("$O(\\\\log n)$", true)], "Halves the range each step.")`,
  steps: `root = Stack([s])
s = Steps("Solve $2x + 3 = 11$", [Step("Subtract 3", "$2x = 8$"), Step("Divide by 2", "$x = 4$")], "$x = 4$")`,
  explorer: `root = Stack([ex])
$p = 1000
$r = 5
$n = 10
ex = Explorer("Compound interest", [Slider($p, "Principal", 100, 10000, 100, "$"), Slider($r, "Rate", 0, 15, 0.5, "%"), Slider($n, "Years", 1, 40, 1, "yr"), Value("Balance", $p * @Pow(1 + $r / 100, $n), "$", 2), Chart(pts, "Years", "Balance")])
pts = @Each(@Range(0, $n), "t", {x: t, y: $p * @Pow(1 + $r / 100, t)})`,
  pageRef: `root = Stack([Text("See the lecture:"), PageRef("courses/MAST10006/Lecture 4.pdf", 12, "Chain rule")])`,
  combined: `root = Stack([intro, deck, quiz, grade, PageRef("courses/COMP20003/Week 3.pdf", 14), Ask("Harder questions", @ToAssistant("Give me five harder questions"))])
$final = 60
intro = Text("Warm up, then test yourself.")
deck = Flashcards("Terms", [Card("Stable sort", "Keeps equal keys in input order")])
quiz = Quiz("Check", [Question("Stable?", [Option("Merge sort", true), Option("Heapsort")])])
grade = Explorer("Grade needed", [Slider($final, "Final exam", 0, 100, 1, "%"), Value("Subject mark", 72 * 0.5 + $final * 0.5, "%", 1)])`,
} as const;

describe("openui programs", () => {
  for (const [name, source] of Object.entries(PROGRAMS)) {
    test(`${name} parses clean`, () => {
      const v = parseProgram(source);
      if (!v.ok) throw new Error(`${v.reason}: ${v.errors.join("; ")}`);
      expect(v.result.root?.typeName).toBe("Stack");
    });
  }

  for (const [name, source] of Object.entries(PROMPT_EXAMPLES)) {
    test(`the prompt's ${name} example parses clean`, () => {
      const v = parseProgram(source);
      if (!v.ok) throw new Error(`${v.reason}: ${v.errors.join("; ")}`);
    });
  }

  test("an unknown component refuses the fence", () => {
    const v = parseProgram(`root = Stack([Map("Melbourne")])`);
    expect(v.ok).toBe(false);
    if (!v.ok) expect(v.errors.join()).toContain("unknown-component");
  });

  test("a fence cut mid-stream is incomplete, not drawn", () => {
    const cut = PROGRAMS.combined.slice(0, PROGRAMS.combined.indexOf("quiz = Quiz"));
    const v = parseProgram(cut);
    expect(v.ok).toBe(false);
    if (!v.ok) expect(v.reason).toBe("incomplete");
  });

  test("a wrong argument type refuses the fence", () => {
    const v = parseProgram(`root = Stack([Quiz("Q", [Question("x", ["a", "b"])])])`);
    expect(v.ok).toBe(false);
    if (!v.ok) expect(v.errors.join()).toContain("type-mismatch");
  });
});

// The widgets themselves aren't rendered here: their text goes through
// `InlineMd`, whose `useMathsReady` has no server snapshot, so a static
// render throws. Drawing is checked in a browser.
describe("openui fence", () => {
  test("only widget fences get a stable syntax-order index", () => {
    const md = "```openui\nroot = Stack([])\n```\n\n```text\nordinary\n```\n\n```openui-lang\nroot = Stack([])\n```";
    const html = renderToStaticMarkup(<ReactMarkdown remarkPlugins={[remarkWidgets]}>{md}</ReactMarkdown>);
    expect(html).toContain('data-widget-index="0"');
    expect(html).toContain('data-widget-index="1"');
    expect(html).not.toContain('data-widget-index="2"');
  });
  test("a fence shows as code until it settles", () => {
    const md = "Try this:\n\n```openui\n" + PROGRAMS.steps + "\n```\n";
    const html = renderToStaticMarkup(<ReactMarkdown components={MD_COMPONENTS}>{md}</ReactMarkdown>);
    expect(html).toContain("<pre");
    expect(html).toContain("Steps(");
  });
});

describe("repairTex", () => {
  test("single-backslash commands eaten by JSON escapes come back", () => {
    expect(repairTex("$\frac{1}{2}$")).toBe("$\\frac{1}{2}$");
    expect(repairTex("$a \times b$")).toBe("$a \\times b$");
    expect(repairTex("$\beta + \rho$")).toBe("$\\beta + \\rho$");
    expect(repairTex("$a \neq b$")).toBe("$a \\neq b$");
  });

  test("a newline in prose stays a newline", () => {
    expect(repairTex("line one\nline two")).toBe("line one\nline two");
  });

  test("valid escaped TeX is untouched", () => {
    expect(repairTex("$\\frac{a}{b} \\times \\beta \\neq \\rho$")).toBe("$\\frac{a}{b} \\times \\beta \\neq \\rho$");
  });

  test("lost backslashes and ambiguous newlines cannot be reconstructed", () => {
    // Commands such as \alpha are invalid JSON escapes: the parser must
    // reject them first. If only their letters survive, repair has no signal.
    expect(repairTex("$alpha + gamma$")).toBe("$alpha + gamma$");
    // A deliberate line break before a letter in maths is indistinguishable
    // from \n; models must still double every TeX backslash.
    expect(repairTex("$a\nnext$")).toBe("$a\\nnext$");
    expect(repairTex("$a\n2$")).toBe("$a\n2$");
  });
});

test("negative chart bars keep their zero baseline within the plot", () => {
  const View = Chart.component;
  const html = renderToStaticMarkup(<View props={{ points: [{ x: 0, y: -10 }, { x: 1, y: -5 }], kind: "bar" }} renderNode={() => null} />);
  const bars = [...html.matchAll(/<rect[^>]* y="([^"]+)"[^>]* height="([^"]+)"/g)];
  expect(bars).toHaveLength(2);
  for (const [, y, height] of bars) {
    expect(Number(y)).toBeGreaterThanOrEqual(8);
    expect(Number(y) + Number(height)).toBeLessThanOrEqual(134);
  }
});
