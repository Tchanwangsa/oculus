---
name: oculus-ui
description: Draws interactive study widgets (flashcards, quizzes, step-by-step worked problems, formula explorers with sliders and charts, links to library pages) in a chat reply by writing a fenced `openui` code block. Use when the student asks to be drilled, quizzed, walked through a problem, or to explore how a formula behaves.
---

# Study widgets

Oculus draws a fenced code block tagged `openui` in your reply as a live study widget. Write it when doing beats reading: flashcards to drill, a quiz to check understanding, a worked problem to try step by step, or a formula to explore with sliders. Everything else in the reply stays ordinary markdown. One widget block per reply, after a sentence or two of prose; never wrap the block in other text or explain its syntax. If the block does not parse, the student sees the raw code, so follow the grammar below exactly.

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
Props typed `ActionExpression` accept one @step, or Action([@steps...]) for several. See the Action section for available steps (@ToAssistant, @Set, @Reset).
Props marked `$binding<type>` accept a `$variable` reference for two-way binding.

### Root
Stack(items: (Flashcards | Quiz | Steps | Explorer | Text | PageRef | Ask)[]) — The root: study widgets top to bottom. Ask and PageRef items line up in a row.
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

## Ask actions

`Ask("Quiz me on these")` sends its label as the student's next message.
`Ask("Harder questions", @ToAssistant("Give me five harder questions"))`
sends the given message. It appears only in a conversation that is idle.
Use at most two, phrased as the student.

`Action([@Set($rate, 5), @ToAssistant("Explain this rate")])` runs steps in
order. `@Set($variable, value)` changes a slider's value and
`@Reset($variable)` restores its declared default. These and `@ToAssistant`
are the available actions. Use `PageRef` for library links.

## Complete programs appear after streaming pauses

Put `root = Stack(...)` first, then $variable defaults, then the widgets and
data. Forward references are allowed. Oculus waits 150 ms after the source
stops changing, and draws only complete, valid programs. Incomplete or invalid
programs stay visible as code. Progress survives reopening the conversation.

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

## Before sending

- Every named reference is defined and reachable from `root`.
- Every `$binding` has a literal numeric default.
- Each quiz question has exactly one correct option.
- Strings are one line, with `\n` for a newline. Double every TeX backslash:
  `"$\\frac{a}{b}$"`. Do not rely on the runtime's limited repair of JSON escapes.
- Cite real library paths. Never invent a page or a component.
- Keep prose outside the fence; use widgets when interaction helps the student.
