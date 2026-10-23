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
const { generateOculusUiPrompt } = await import("@/components/markdown/openui/prompt");
const { includeChange, conflicts, effectiveChanges, initialProposal, validateProposal } = await import("@/components/markdown/openui/proposals/model");
const { viewRows, replaceAt } = await import("@/components/markdown/openui/proposals/ProposalRows");
const { DateTimeField } = await import("@/components/projects/fields/DateTimeField");
const { displayValue } = await import("@/components/markdown/openui/proposals/ProposalParts");

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

describe("proposal signatures", () => {
  const examples = [
    `NewTasks(null, [Task("Outline", null, 30, null, [Task("Read")]), Under(2, [Task("Check")])])`,
    `NewProject("Exam prep", null, [Task("Practice")], null, "A plan", null, ["Exam"], null, "812.0")`,
    `NewEvents([Event("Practice", "2026-10-27", null, "note", null, "Bring notes")])`,
    `NewNote("COMP30026", "Summary", "The complete body")`,
    `TaskChanges([Change(42, {due: "2026-10-27", estimate: 30}, {due: "2026-10-30", estimate: 60})])`,
    `ProjectChanges([Change(12, {status: "active"}, {status: "archived"})])`,
    `EventChanges([Change("local_5", {start: "2026-10-27"}, {start: "2026-10-28"})])`,
    `TagChanges([NewTag("Exam", "indigo"), Change(2, {color: "slate"}, {color: "green"}), DeleteTag(3)])`,
    `DeleteTasks([1, 2])`, `DeleteEvents(["local_5"])`, `Sync(["COMP30026"], null)`, `Reparse(["courses/COMP30026/Week1.pdf"])`, `Transcribe("courses/COMP30026/Week1.mp4")`, `LectureChapters("lecture-1")`, `LectureEnd("lecture-1")`, `Download("aligner")`,
  ];
  for (const component of examples) test(component.split("(")[0] + " signature parses, including null skips", () => {
    const verdict = parseProgram(`root = Stack([${component}])`);
    if (!verdict.ok) throw new Error(verdict.errors.join("; "));
    expect(verdict.ok).toBe(true);
  });
  test("generated grammar keeps null-skip optional markers and recursive Task type", () => {
    const prompt = generateOculusUiPrompt();
    expect(prompt).toContain("Task(title: string, due?: string, estimate?: number, column?: string, subtasks?: Task[]");
    expect(prompt).toContain("NewTasks(project: number | null, items: (Task | Under)[], amends?: string)");
    expect(prompt).toContain("NewProject(name: string, subject: string | null,");
    expect(prompt).toContain("Value(label: string, value: number | null,");
    expect(prompt).toContain("y: number | null");
  });
  for (const source of [
    `TaskChanges([Change(1, {due: "2026-10-27"}, {title: "Other"})])`,
    `TaskChanges([Change(1, {surprise: "a"}, {surprise: "b"})])`,
    `EventChanges([Change("canvas_5", {title: "Old"}, {title: "New"})])`,
    `DeleteEvents(["canvas_5"])`,
    `NewTasks(null, [Task("Parent", null, null, null, [Task("Child", null, null, null, [Task("Grandchild")])])])`,
    `NewTasks(null, [Under(1, [Task("Child", null, null, null, [Task("Grandchild")])])])`,
    `NewEvents([Event("Wrong span", "2026-10-27", "2026-10-28", "note")])`,
    `NewTasks(null, [Task("Wrong date", "tomorrow")])`,
  ]) test("malformed proposal stays code: " + source.slice(0, 70), () => expect(parseProgram(`root = Stack([${source}])`).ok).toBe(false));
  test("two proposals or jobs in one fence refuse the complete fence", () => {
    expect(parseProgram(`root = Stack([NewTasks(null, [Task("A")]), DeleteTasks([1])])`).ok).toBe(false);
    expect(parseProgram(`root = Stack([Sync(["COMP30026"]), Download("aligner")])`).ok).toBe(false);
  });
  test("the required null-capable slots cannot be omitted from decoded state", () => {
    expect(() => validateProposal("NewTasks", { items: [] })).toThrow("requires project");
    expect(() => validateProposal("NewProject", { name: "Project", tasks: [] })).toThrow("requires subject");
    expect(() => validateProposal("NewTasks", { project: null })).toThrow("requires items");
    expect(() => validateProposal("NewNote", { subject: "X", title: "Title" })).toThrow("requires subject, title and body");
  });
  test("explicitly including a stale change rebases only its compared fields", () => {
    const original = { id: 1, from: { due: "2026-10-27" }, to: { due: "2026-10-30" }, included: false };
    const live = { due: "2026-10-28", title: "User's current title" };
    expect(conflicts(original, live)).toEqual(["due"]);
    const selected = includeChange(original, true, live);
    expect(selected).toEqual({ ...original, from: { due: "2026-10-28" }, included: true });
    expect(conflicts(selected, live)).toEqual([]);
    expect(original.from.due).toBe("2026-10-27");
    expect(effectiveChanges({ from: { due: null }, to: { due: null } })).toEqual({});
  });
  test("delete exclusions become durable row state", () => {
    expect(initialProposal("DeleteEvents", { events: ["local_5"] })).toEqual({ events: [{ id: "local_5", included: true }] });
  });
  test("paging rows retains exclusions and edits on other pages, including grouped subtasks", () => {
    const original = { project: null, items: Array.from({ length: 7 }, (_, i) => ({ title: `Task ${i + 1}` })) };
    const listing = viewRows("NewTasks", original, "items");
    const excluded = replaceAt(original, listing[0].path, { ...listing[0].row, included: false });
    const edited = replaceAt(excluded, listing[5].path, { ...listing[5].row, title: "Edited on page two" });
    const remounted = viewRows("NewTasks", edited, "items");
    expect(remounted.slice(0, 5)[0].row.included).toBe(false);
    expect(remounted.slice(5)[0].row.title).toBe("Edited on page two");
    expect(original.items[0]).toEqual({ title: "Task 1" });
    expect(viewRows("NewTasks", { items: [{ task: 12, subtasks: [{ title: "Child A" }, { title: "Child B" }] }] }, "items").map((item) => item.parent?.task)).toEqual([12, 12]);
  });
  test("date-only proposals show a calendar day without an invented clock time", () => {
    const html = renderToStaticMarkup(<DateTimeField value="2026-10-27" label="Due" onCommit={() => {}} allowAllDay />);
    expect(html).not.toMatch(/\d{1,2}:\d{2}/);
    expect(displayValue("2026-10-27", "due")).not.toMatch(/\d{1,2}:\d{2}/);
    expect(displayValue("2026-10-27", "due")).toContain("27");
  });
  test("required nullable study values accept null without making their fields omittable", () => {
    expect(parseProgram(`root = Stack([Explorer("Undefined", [Value("No result", null), Chart([{x:0,y:null},{x:1,y:2}])])])`).ok).toBe(true);
    expect(parseProgram(`root = Stack([Explorer("Missing", [Value("No result")])])`).ok).toBe(false);
    expect(parseProgram(`root = Stack([Explorer("Missing", [Chart([{x:0},{x:1,y:2}])])])`).ok).toBe(false);
  });
});
