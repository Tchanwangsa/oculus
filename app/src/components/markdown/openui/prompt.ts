import type { PromptOptions } from "@openuidev/react-lang";
import { library } from "./library";

/**
 * What the generated prompt says beyond the library's signatures. The stock
 * preamble demands a reply that is nothing but OpenUI Lang; Oculus replies
 * are markdown, and a widget is one fenced block inside one.
 */

const PREAMBLE = `Oculus draws a fenced code block tagged \`openui\` in your reply as a live study widget or a proposal the student reviews and confirms. Use study widgets when doing beats reading: flashcards to drill, a quiz to check understanding, a worked problem to try step by step, or a formula to explore with sliders. Always use a proposal before creating, changing or deleting the student's tasks, projects, tags, calendar events or notes, or starting a long or billed job. Nothing is written until the student clicks. Everything else in the reply stays ordinary markdown. Keep surrounding prose to a sentence or two. Each proposal or job needs its own fence; study widgets may share a Stack. If a block does not parse, the student sees the raw code, so follow the grammar below exactly.

The block holds a program in OpenUI Lang:`;

const RULES = [
  "Start the program with `root = Stack([...])` and list every widget in it.",
  "Strings are double-quoted on one line; write a newline as \\n and a double quote as \\\". Maths goes in $…$ inside strings, e.g. \"$\\\\frac{a}{b}$\" (backslashes doubled).",
  "Take facts from the student's own library when you can, and point to the page with PageRef using the same path you would cite in prose.",
  "$variables are only for Explorer: declare each with a literal number default, e.g. `$rate = 5`, and bind it as a Slider's first argument.",
  "Ask buttons are optional follow-ups the student may want next, phrased as the student (\"Quiz me on the ones I missed\"). At most two.",
  "Task(number) reads a live existing task; Task(string, ...) describes a new task inside a proposal. Read ids with task list, lecture list and studio list; never guess. After explaining a concept, offer condensed CheatEntry items in NewCheatNotes, never the entire reply or task/sync chatter.",
  "Never use a component or @function not listed here.",
  "Optional arguments may be null to skip one in the middle. Project/task/tag ids are numbers, subjects are course code strings or null for personal, calendar ids are strings; only local_<id> events may be changed or deleted. Look up existing ids through the CLI, never guess.",
  "Dates are ISO 8601; estimates are whole minutes. Tags are arrays of names. Task subtasks nest only one level. Event kind defaults to note; only class may have an end.",
  "Change from and to must have exactly the same permitted keys. Use the values read from the CLI in from, replacements in to; null clears a field.",
  "To amend a pending card, pass the app's id from Cards in this thread as the last argument. Preserve the student's edits and excluded rows; decided cards need a fresh Changes proposal on the made ids.",
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

const NEW_TASKS = `root = Stack([NewTasks(12, [Task("Read the specification", "2026-10-27", 30), Task("Implement the parser", "2026-10-28", 120, null, [Task("Handle malformed input")])])])`;
const CHANGES = `root = Stack([TaskChanges([Change(42, {due: "2026-10-27"}, {due: "2026-10-30"})])])`;
const NEW_EVENTS = `root = Stack([NewEvents([Event("Past-paper practice", "2026-10-27T14:00:00+11:00", "2026-10-27T16:00:00+11:00", "class", "COMP30026")])])`;

export const PROMPT_OPTIONS: PromptOptions = {
  preamble: PREAMBLE,
  bindings: true,
  additionalRules: RULES,
  examples: [
    `Flashcards with a follow-up:\n\n\`\`\`openui\n${FLASHCARDS}\n\`\`\``,
    `A quiz with a source page:\n\n\`\`\`openui\n${QUIZ}\n\`\`\``,
    `A worked problem:\n\n\`\`\`openui\n${STEPS}\n\`\`\``,
    `A formula explorer:\n\n\`\`\`openui\n${EXPLORER}\n\`\`\``,
    `Tasks to review and create:\n\n\`\`\`openui\n${NEW_TASKS}\n\`\`\``,
    `A guarded date change:\n\n\`\`\`openui\n${CHANGES}\n\`\`\``,
    `A study session on the calendar:\n\n\`\`\`openui\n${NEW_EVENTS}\n\`\`\``,
  ],
};

/** The example programs, for tests: each must parse. */
export const PROMPT_EXAMPLES = { FLASHCARDS, QUIZ, STEPS, EXPLORER, NEW_TASKS, CHANGES, NEW_EVENTS };

/** The 1.0.1 printer loses a lazy recursive ref and treats nullable required
 * slots as omittable. Runtime validation enforces these exact signatures. */
export function generateOculusUiPrompt(): string {
  return library.prompt(PROMPT_OPTIONS)
    .replace("NewTasks(project?: number,", "NewTasks(project: number | null,")
    .replace("NewProject(name: string, subject?: string,", "NewProject(name: string, subject: string | null,")
    .replace("subtasks?: any[]", "subtasks?: Task[]")
    .replace("Value(label: string, value?: number,", "Value(label: string, value: number | null,")
    .replace("y?: number", "y: number | null");
}
