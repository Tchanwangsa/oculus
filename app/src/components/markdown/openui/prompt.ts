import type { PromptOptions } from "@openuidev/react-lang";

/**
 * What the generated prompt says beyond the library's signatures. The stock
 * preamble demands a reply that is nothing but OpenUI Lang; Oculus replies
 * are markdown, and a widget is one fenced block inside one.
 */

const PREAMBLE = `Oculus draws a fenced code block tagged \`openui\` in your reply as a live study widget. Write it when doing beats reading: flashcards to drill, a quiz to check understanding, a worked problem to try step by step, or a formula to explore with sliders. Everything else in the reply stays ordinary markdown. One widget block per reply, after a sentence or two of prose; never wrap the block in other text or explain its syntax. If the block does not parse, the student sees the raw code, so follow the grammar below exactly.

The block holds a program in OpenUI Lang:`;

const RULES = [
  "Start the program with `root = Stack([...])` and list every widget in it.",
  "Strings are double-quoted on one line; write a newline as \\n and a double quote as \\\". Maths goes in $…$ inside strings, e.g. \"$\\\\frac{a}{b}$\" (backslashes doubled).",
  "Take facts from the student's own library when you can, and point to the page with PageRef using the same path you would cite in prose.",
  "$variables are only for Explorer: declare each with a literal number default, e.g. `$rate = 5`, and bind it as a Slider's first argument.",
  "Ask buttons are optional follow-ups the student may want next, phrased as the student (\"Quiz me on the ones I missed\"). At most two.",
  "Never use a component or @function not listed here.",
];

const FLASHCARDS = `root = Stack([deck, Ask("Quiz me on these")])
deck = Flashcards("Glycolysis", [c1, c2, c3])
c1 = Card("Net ATP from glycolysis?", "**2 ATP** per glucose (4 made, 2 spent)")
c2 = Card("Rate-limiting enzyme?", "Phosphofructokinase-1 (PFK-1)")
c3 = Card("End product under aerobic conditions?", "Two pyruvate, which enter the mitochondria")`;

const QUIZ = `root = Stack([quiz, PageRef("courses/COMP20003/Week 3 Sorting.pdf", 14)])
quiz = Quiz("Sorting complexity", [q1, q2])
q1 = Question("Worst case of quicksort with a first-element pivot?", [Option("$O(n \\\\log n)$"), Option("$O(n^2)$", true), Option("$O(n)$")], "An already-sorted input splits $n-1$ / $0$ every time.")
q2 = Question("Which sort is stable?", [Option("Heapsort"), Option("Merge sort", true), Option("Quicksort")])`;

const STEPS = `root = Stack([work])
work = Steps("Solve $3x - 4 = 11$", [Step("Add 4 to both sides", "$3x = 15$"), Step("Divide by 3", "$x = 5$"), Step("Check", "$3(5) - 4 = 11$ ✓")], "$x = 5$")`;

const EXPLORER = `root = Stack([ex])
$m = 2
$v = 3
ex = Explorer("Kinetic energy", [Text("$E_k = \\\\tfrac{1}{2} m v^2$"), Slider($m, "Mass", 0.5, 10, 0.5, "kg"), Slider($v, "Speed", 0, 20, 0.5, "m/s"), Value("Kinetic energy", 0.5 * $m * @Pow($v, 2), "J", 1), Chart(curve, "Speed (m/s)", "Energy (J)")])
curve = @Each(@Range(0, 20), "s", {x: s, y: 0.5 * $m * @Pow(s, 2)})`;

export const PROMPT_OPTIONS: PromptOptions = {
  preamble: PREAMBLE,
  bindings: true,
  additionalRules: RULES,
  examples: [
    `Flashcards with a follow-up:\n\n\`\`\`openui\n${FLASHCARDS}\n\`\`\``,
    `A quiz with a source page:\n\n\`\`\`openui\n${QUIZ}\n\`\`\``,
    `A worked problem:\n\n\`\`\`openui\n${STEPS}\n\`\`\``,
    `A formula explorer:\n\n\`\`\`openui\n${EXPLORER}\n\`\`\``,
  ],
};

/** The example programs, for tests: each must parse. */
export const PROMPT_EXAMPLES = { FLASHCARDS, QUIZ, STEPS, EXPLORER };
