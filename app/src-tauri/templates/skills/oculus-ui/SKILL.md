---
name: oculus-ui
description: Draws interactive study widgets and editable proposal cards in an Oculus chat using fenced OpenUI programs. Use for flashcards, quizzes, worked problems, formula explorers, creating or changing tasks, projects, tags, calendar events or notes, and confirming sync, parse, transcription, lecture or download jobs.
---

# Chat widgets and proposal cards

Oculus draws a fenced code block tagged `openui` in your reply as a live study widget or a proposal the student reviews and confirms. Use study widgets when doing beats reading: flashcards to drill, a quiz to check understanding, a worked problem to try step by step, or a formula to explore with sliders. Always use a proposal before creating, changing or deleting the student's tasks, projects, tags, calendar events or notes, or starting a long or billed job. Nothing is written until the student clicks. Everything else in the reply stays ordinary markdown. Keep surrounding prose to a sentence or two. Each proposal or job needs its own fence; study widgets may share a Stack. If a block does not parse, the student sees the raw code, so follow the grammar below exactly.

The block holds a program in OpenUI Lang:

## Syntax Rules

1. Each statement is on its own line: `identifier = Expression`
2. `root` is the entry point — every program must define `root = Stack(...)`
3. Expressions are: strings ("..."), numbers, booleans (true/false), null, arrays ([...]), objects ({...}), or component calls TypeName(arg1, arg2, ...)
4. Use references for readability: define `name = ...` on one line, then use `name` later
5. EVERY variable (except root) MUST be referenced by at least one other variable. Unreferenced variables are silently dropped and will NOT render. Always include defined variables in their parent's children/items array.
6. Arguments are POSITIONAL (order matters, not names). Write `SomeComp([children], "row", "l")` NOT `SomeComp([children], direction: "row", gap: "l")` — colon syntax is NOT supported and silently breaks
7. Optional arguments can be omitted from the end
8. Declare mutable state with `$varName = defaultValue`. Components marked with `$binding` can read/write these. Undeclared $variables are auto-created with null default.
9. String concatenation: `"text" + $var + "more"`
10. Dot member access: `query.field` reads a field; on arrays it extracts that field from every element
11. Index access: `arr[0]`, `data[index]`
12. Arithmetic operators: +, -, *, /, % (work on numbers; + is string concat when either side is a string)
13. Comparison: ==, !=, >, <, >=, <=
14. Logical: &&, ||, ! (prefix)
15. Ternary: `condition ? valueIfTrue : valueIfFalse`
16. Parentheses for grouping: `(a + b) * c`
- Strings use double quotes with backslash escaping

## Component Signatures

Arguments marked with ? are optional. Sub-components can be inline or referenced; prefer references for better streaming.
Props typed `ActionExpression` accept one @step, or Action([@steps...]) for several. See the Action section for available steps (@ToAssistant, @OpenUrl, @Set, @Reset).
Props marked `$binding<type>` accept a `$variable` reference for two-way binding.

### Root
Stack(items: (Flashcards | Quiz | Steps | Explorer | Text | PageRef | Ask | NewTasks | NewProject | NewEvents | NewNote | TaskChanges | ProjectChanges | EventChanges | TagChanges | DeleteTasks | DeleteEvents | Sync | Reparse | Transcribe | LectureChapters | LectureEnd | Download)[]) — The root: study widgets top to bottom. Ask and PageRef items line up in a row.
Text(text: string, tone?: "normal" | "muted") — A short line of markdown; $…$ for maths. Strings may be built with + from $variables.

### Recall
Flashcards(title: string, cards: Card[]) — A flashcard deck: flip, then mark each card known or not; unknown cards come round again.
Card(front: string, back: string) — One flashcard: a prompt on the front, the answer on the back. Only inside Flashcards.
Quiz(title: string, questions: Question[]) — A multiple-choice quiz, one question at a time, scored as it goes.
Question(prompt: string, options: Option[], explanation?: string) — A multiple-choice question. Only inside Quiz.
Option(text: string, correct?: boolean) — One answer choice; mark exactly one option per question correct = true. Only inside Question.
- Mark exactly one answer per question with Option(text, true).
- 5 to 12 cards or 3 to 8 questions; every one checks a single idea.

### Working
Steps(problem: string, steps: Step[], answer?: string) — A worked problem revealed one step at a time; answer is shown after the last step.
Step(title: string, work?: string) — One step of a worked solution. Only inside Steps.

### Explore
Explorer(title: string, items: (Slider | Value | Chart | Text)[]) — A card of Sliders bound to $variables, with Values and Charts computed from them, plus Text for the formula.
Slider(value: $binding<number>, label: string, min: number, max: number, step?: number, unit?: string) — Drags a $variable between min and max; unit like "%", "$" or "yr" formats the readout.
Value(label: string, value: number | null, unit?: string, decimals?: number) — A computed number shown large, e.g. Value("Balance", $p * @Pow(1 + $r / 100, $n), "$", 2).
Chart(points: {x: number, y: number | null}[], xLabel?: string, yLabel?: string, kind?: "line" | "bar") — A line (default) or bar chart of {x, y} points, usually computed from sliders with @Each(@Range(...), "t", {x: t, y: ...}).
- A Slider's first argument is a $variable declared with a literal default; Values and Charts compute from those $variables.
- There is no ^ operator: write @Pow(base, exponent).

### Links
PageRef(path: string, page?: number, label?: string) — A link to a page of a library file, e.g. PageRef("courses/MAST10006/Lecture 4.pdf", 12, "Chain rule"); same paths you cite in prose.
Ask(label: string, action?: ActionExpression) — A button that sends a follow-up message as the student, e.g. Ask("Harder questions", @ToAssistant("Give me five harder Big-O questions")). Without an action it sends its label.

### Create
NewTasks(project: number | null, items: (Task | Under)[], amends?: string) — Propose creating tasks in an existing project, or null for unfiled. The student edits and confirms.
NewProject(name: string, subject: string | null, tasks: Task[], due?: string, brief?: string, starts?: string, tags?: string[], event?: string, amends?: string) — Propose a new project and its tasks together. Nothing is written until Create.
Task(title: string, due?: string, estimate?: number, column?: string, subtasks?: Task[], tags?: string[], body?: string, starts?: string, files?: string[]) — A new task; subtasks may nest only one level. Used only inside a creation card.
Under(task: number, subtasks: Task[]) — New subtasks grouped under an existing task; only in NewTasks.
NewEvents(events: Event[], amends?: string) — Propose the student's own calendar events, confirmed together.
Event(title: string, start: string, end?: string, kind?: "note" | "due" | "class", subject?: string, notes?: string) — A proposed calendar event. A study session uses kind class and an end. Other kinds cannot have an end.
NewNote(subject: string, title: string, body: string, amends?: string) — Propose a note in a subject, with its complete markdown body.
- Use one proposal per openui fence. Nothing is written until the student clicks Create.
- Look up existing ids in the CLI; never guess. Optional arguments may be null to skip a middle argument.
- amends is the app's pending card id from Cards in this thread, always the last argument.

### Change
Change(id: number | string, from: Record<string, any>, to: Record<string, any>) — An existing id, the exact fields read from it, and replacement values for those same fields. Null clears a field.
TaskChanges(changes: Change[], amends?: string) — Propose task field changes, moves or refile. Each Change carries matching from and to keys.
ProjectChanges(changes: Change[], amends?: string) — Propose changes to projects, including archiving.
EventChanges(changes: Change[], amends?: string) — Propose changes only to the student's local_<id> events; Canvas events are read-only.
TagChanges(changes: (Change | NewTag | DeleteTag)[], amends?: string) — Propose shared tag changes, creations or removals.
NewTag(name: string, color: "slate" | "indigo" | "green" | "amber" | "rose" | "purple" | "teal" | "orange") — A proposed shared tag and its palette colour key.
DeleteTag(tag: number) — Remove a shared tag by id, including its links on tasks and projects.
DeleteTasks(tasks: number[], amends?: string) — Propose deleting tasks and their subtasks; Undo restores them if nothing has changed.
DeleteEvents(events: string[], amends?: string) — Propose deleting only the student's local_<id> calendar events.
- Change(id, from, to) uses the same keys in from and to. Null clears a field.
- Task keys: title, body, due, starts, estimate, tags, files, column, project. Project keys: name, subject, due, starts, brief, tags, event, status. Event keys: title, start, end, kind, subject, notes. Tag keys: name, color.
- Only local_<id> events may be changed/deleted; Canvas events are read-only.

### Jobs
Sync(subjects: string[], lectures?: boolean) — Ask before syncing subjects and optionally lectures.
Reparse(files: string[]) — Ask before reparsing and embedding the listed PDF library paths.
Transcribe(file: string) — Ask before transcribing a media file using the configured engine.
LectureChapters(lecture: string) — Ask before finding chapters using the configured job model.
LectureEnd(lecture: string) — Ask before finding a lecture's content end using the configured model.
Download(item: string) — Ask before downloading aligner, voices starter pack or a voice id.
- Ask with a job card before starting long, billed or downloaded work. One job per fence. No Undo.

## Built-in Functions

Data functions prefixed with `@` to distinguish from components. These are the ONLY functions available — do NOT invent new ones.
Use @-prefixed built-in functions (@Count, @Sum, @Avg, @Min, @Max, @Round) on data. Do NOT hardcode computed values.

@Count(array: any[]) → number — Returns array length
@First(array: any[]) → any — Returns first element of array
@Last(array: any[]) → any — Returns last element of array
@Sum(numbers: number[]) → number — Sum of numeric array
@Avg(numbers: number[]) → number — Average of numeric array
@Min(numbers: number[]) → number — Minimum value in array
@Max(numbers: number[]) → number — Maximum value in array
@Sort(array: any[], field: string, direction?: "asc" | "desc") → any[] — Sort array by field. Direction: "asc" (default) or "desc"
@Filter(array: any[], field: string, operator: "==" | "!=" | ">" | "<" | ">=" | "<=" | "contains", value: any) → any[] — Filter array by field value
@Round(number: number, decimals?: number) → number — Round to N decimal places (default 0)
@Abs(number: number) → number — Absolute value
@Floor(number: number) → number — Round down to nearest integer
@Ceil(number: number) → number — Round up to nearest integer
@Each(array: any[], varName: string, template: any) — Evaluate template for each element. varName is the loop variable — use it ONLY inside the template expression (inline). Do NOT create a separate statement for the template.
@Pow(base: number, exponent: number) → number — base raised to exponent, e.g. @Pow(1.05, 10)
@Sqrt(number: number) → number — Square root; null for a negative number
@Log(number: number) → number — Natural logarithm; null for zero or less
@Range(start: number, end: number, step?: number) → number[] — Numbers from start to end inclusive, stepping by step (default 1), e.g. @Range(0, 10) for a chart's x values; at most 400

Builtins compose — output of one is input to the next:
`@Count(@Filter(data.rows, "field", "==", "val"))` for KPIs/chart values, `@Round(@Avg(data.rows.score), 1)`, `@Each(data.rows, "item", Comp(item.field))` for per-item rendering.
Array pluck: `data.rows.field` extracts a field from every row → use with @Sum, @Avg, charts, tables.

IMPORTANT @Each rule: The loop variable (e.g. "item") is ONLY available inside the @Each template expression. Always inline the template — do NOT extract it to a separate statement.
CORRECT: `list = @Each(rows, "t", Comp(t.name))`
WRONG: `item = Comp(t.name)` then `list = @Each(rows, "t", item)`, because t is undefined in item.

## Action — Button Behavior

An action wires a button click to operations. Pass one @-prefixed step bare, e.g. Button("Ask", @ToAssistant("Tell me more")). Put several steps in Action([...]), e.g. Button("Read", Action([@OpenUrl("https://example.com"), @ToAssistant("Summarize this page")])). Steps execute in order.
Buttons without an action automatically send their label to the assistant (equivalent to @ToAssistant(label)).

Available steps:
- @ToAssistant(message: string, context?: any) — Send a message to the assistant (for conversational buttons like "Tell me more", "Explain this"). The optional context is any value (object, array, string, number) passed to the assistant as hidden data, e.g. @ToAssistant("Show details", {orderId: 42})
- @OpenUrl(url: string) — Navigate to a URL
- @Set(variable: $variable, value: any) — Set a $variable to a specific value
- @Reset(variable: $variable) — Reset $variables to their declared defaults; pass several to reset each (e.g. @Reset($title, $priority) restores $title="" and $priority="medium")

Example — simple nav:
```
viewBtn = Button("View", @OpenUrl("https://example.com"))
```

- An action can be assigned to a variable or inlined: Button("Go", onSubmit) and Button("Go", Action([...])) both work

## Hoisting & Streaming (CRITICAL)

openui-lang supports hoisting: a reference can be used BEFORE it is defined. The parser resolves all references after the full input is parsed.

During streaming, the output is re-parsed on every chunk. Undefined references are temporarily unresolved and appear once their definitions stream in. This creates a progressive top-down reveal — structure first, then data fills in.

**Recommended statement order for optimal streaming:**
1. `root = Stack(...)` — UI shell appears immediately
2. $variable declarations — state ready for bindings
3. Component definitions — fill in with data already available
4. Data values — leaf content last

Always write the root = Stack(...) statement first so the UI shell appears immediately, even before child data has streamed in.

## Examples

Flashcards with a follow-up:

```openui
root = Stack([deck, Ask("Quiz me on these")])
deck = Flashcards("Glycolysis", [c1, c2, c3])
c1 = Card("Net ATP from glycolysis?", "**2 ATP** per glucose (4 made, 2 spent)")
c2 = Card("Rate-limiting enzyme?", "Phosphofructokinase-1 (PFK-1)")
c3 = Card("End product under aerobic conditions?", "Two pyruvate, which enter the mitochondria")
```

A quiz with a source page:

```openui
root = Stack([quiz, PageRef("courses/COMP20003/Week 3 Sorting.pdf", 14)])
quiz = Quiz("Sorting complexity", [q1, q2])
q1 = Question("Worst case of quicksort with a first-element pivot?", [Option("$O(n \\log n)$"), Option("$O(n^2)$", true), Option("$O(n)$")], "An already-sorted input splits $n-1$ / $0$ every time.")
q2 = Question("Which sort is stable?", [Option("Heapsort"), Option("Merge sort", true), Option("Quicksort")])
```

A worked problem:

```openui
root = Stack([work])
work = Steps("Solve $3x - 4 = 11$", [Step("Add 4 to both sides", "$3x = 15$"), Step("Divide by 3", "$x = 5$"), Step("Check", "$3(5) - 4 = 11$ ✓")], "$x = 5$")
```

A formula explorer:

```openui
root = Stack([ex])
$m = 2
$v = 3
ex = Explorer("Kinetic energy", [Text("$E_k = \\tfrac{1}{2} m v^2$"), Slider($m, "Mass", 0.5, 10, 0.5, "kg"), Slider($v, "Speed", 0, 20, 0.5, "m/s"), Value("Kinetic energy", 0.5 * $m * @Pow($v, 2), "J", 1), Chart(curve, "Speed (m/s)", "Energy (J)")])
curve = @Each(@Range(0, 20), "s", {x: s, y: 0.5 * $m * @Pow(s, 2)})
```

Tasks to review and create:

```openui
root = Stack([NewTasks(12, [Task("Read the specification", "2026-10-27", 30), Task("Implement the parser", "2026-10-28", 120, null, [Task("Handle malformed input")])])])
```

A guarded date change:

```openui
root = Stack([TaskChanges([Change(42, {due: "2026-10-27"}, {due: "2026-10-30"})])])
```

A study session on the calendar:

```openui
root = Stack([NewEvents([Event("Past-paper practice", "2026-10-27T14:00:00+11:00", "2026-10-27T16:00:00+11:00", "class", "COMP30026")])])
```

## Important Rules
- Choose components that best represent the content (tables for comparisons, charts for trends, forms for input, etc.)

## Final Verification
Before finishing, walk your output and verify:
1. root = Stack(...) is the FIRST line (for optimal streaming).
2. Every referenced name is defined. Every defined name (other than root) is reachable from root.
3. Every $binding appears in at least one component or expression.

- Start the program with `root = Stack([...])` and list every widget in it.
- Strings are double-quoted on one line; write a newline as \n and a double quote as \". Maths goes in $…$ inside strings, e.g. "$\\frac{a}{b}$" (backslashes doubled).
- Take facts from the student's own library when you can, and point to the page with PageRef using the same path you would cite in prose.
- $variables are only for Explorer: declare each with a literal number default, e.g. `$rate = 5`, and bind it as a Slider's first argument.
- Ask buttons are optional follow-ups the student may want next, phrased as the student ("Quiz me on the ones I missed"). At most two.
- Never use a component or @function not listed here.
- Optional arguments may be null to skip one in the middle. Project/task/tag ids are numbers, subjects are course code strings or null for personal, calendar ids are strings; only local_<id> events may be changed or deleted. Look up existing ids through the CLI, never guess.
- Dates are ISO 8601; estimates are whole minutes. Tags are arrays of names. Task subtasks nest only one level. Event kind defaults to note; only class may have an end.
- Change from and to must have exactly the same permitted keys. Use the values read from the CLI in from, replacements in to; null clears a field.
- To amend a pending card, pass the app's id from Cards in this thread as the last argument. Preserve the student's edits and excluded rows; decided cards need a fresh Changes proposal on the made ids.
