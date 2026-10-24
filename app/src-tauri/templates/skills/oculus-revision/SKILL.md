---
name: oculus-revision
description: Drafts course-style practice questions, model answers and rubrics for the student's Revision queue, and proposes condensed cheat sheet entries after explaining concepts.
---

# Revision

For practice questions, first find the subject's own problem sources with
`oculus search`, `oculus files` and the library listing. Read assignments,
Canvas quizzes, tutorial/workshop sheets, past exams and Ed Lessons challenges.
Use `oculus read` to inspect the questions and their mark allocations.

Match the course's notation, question shape and mark allocation: multi-part
calculations, show-that proofs, code tracing, short explanations, units.
Paraphrase and change values; never copy a source question verbatim. Cite the
problem you modelled it on in `style_ref` as a library path with page/line.
If no problem source is available, say so and leave `style_ref` null rather
than inventing a course style or citation.

Each item has `kind` (card, choice, numeric, short or structured), `prompt`,
`answer` (the model answer), `level`, and shared bucket ids in `topic_ids`.
The old `topic` labels remain available as source metadata; use `topic_ids` for
Revision's shared topic filters.
Choice `options` is an array of strings and `answer` is the exact correct
option text. Numeric `answer` is a finite number written as text; `tolerance`
is an absolute nonnegative number. Short/structured answers have a model answer
and, where there are separate marking points, `rubric: [{point, marks}]`.
Keep rubric marks aligned to the course's examples.

Difficulty is fixed: easy = recall or definition; medium = one method applied,
tutorial level; hard = multi-step or unseen, exam level. A paraphrased exam
question is at least medium. `fsrs_difficulty` belongs to the scheduler and is
never the human difficulty label.

Everything in Revision belongs to one subject: every write names it with
`--subject CODE` (the thread's subject, or the one the student names). There
is no subject-less "General" scope; if the subject is unclear, ask.

File everything under a named topic: the concept or method it belongs to
("Grover's algorithm", "Differentiation"), shared by related entries.
Unsorted is only for something that fits no topic, never a default. The
topic carries the grouping, so titles never repeat it ("The angle θ", not
"Grover: the angle θ"). Before creating material, read
`oculus topic list --subject CODE` and reuse a topic whose name fits.

- In a `NewCheatNotes` card, name topics in `CheatEntry`'s `topics`. A new
  name is created in the entry's subject when the student clicks Add, so
  never run `oculus topic add` for a card.
- For CLI writes (`quiz add`, `quiz set add`, `artifact save`), which take
  ids, create a missing topic first with
  `oculus topic add --name "Differentiation" --subject CODE` and pass the id
  it returns. That is part of the write, not a separate change to ask about.

Topics belong to the entry's subject. An entry can have several named topics;
an empty `topic_ids` array means Unsorted. Never invent a topic id.

Write a JSON-lines batch under `agents/`, then use
`oculus quiz add --subject CODE --batch path`. Agent items are drafts. Tell the
student to accept, edit or delete them in Revision → Quizzes before Review.
A named set can be created with
`oculus quiz set add --title "Week 5" --subject CODE --items 1,2`.
Use `--topics 2,3` on single-question creation or set creation; batch
questions carry `"topic_ids":[2,3]`. Read ids from `oculus quiz list`; never
guess them. Review counts come from
`oculus quiz list --due`; the CLI never changes the FSRS schedule.

After explaining an idea, definition, method or formula, offer a condensed
cheat sheet entry with `NewCheatNotes` from `oculus-ui`: the definition, formula
and one small example, rather than the whole reply. Pass the subject code;
null files the entry under the thread's subject, and a thread without one
needs an explicit code. Do this when the student says "remember this" or
"add that to my cheat sheet" too. Never offer entries for task planning, sync
or chat about the app. The student edits the card and clicks Add; do not run
`oculus cheat add` yourself. `oculus cheat list/show` reads existing entries;
existing entries are edited through `EditItems` below. The arguments are
`CheatEntry(title, body, subject, tags, source, topics)`. Tags come from the
cheat sheet's registry: read it with `oculus cheat tags` and spell a name as
listed (`Formula`, `Definition`, `Worked example`). A new name joins the
registry, so add one only when nothing fits. The library path or page the
entry came from goes in `source`, never in tags, and the last argument is its
topic names.
For example:

```openui
root = Stack([NewCheatNotes([CheatEntry("Product rule", "For $y=uv$, $y'=u'v+uv'$. Example: $(x^2 e^x)'=2xe^x+x^2e^x$.", "MAST10006", ["Formula"], "courses/MAST10006_2026_SM1/files/Lecture_5.pdf#page=3", ["Differentiation"])])])
```

Quiz and Flashcards widgets have Save to Revision, saving drafts. Interactive
fences can be saved as artifacts, retaining their current slider/card state.
`oculus artifact save --title "Explorer" --program-file path` creates an
artifact directly. Use only the widget components in `oculus-ui`; do not
invent iframe or arbitrary HTML components.

## Edit content and topic memberships together

Topic assignment is an ordinary field edit. Read the entry first with
`oculus cheat show`, `oculus quiz show`, `oculus quiz set show`, or
`oculus artifact show`; read topics with `oculus topic list/show`. A list can
be narrowed by `--topic ID` on cheat, quiz, quiz set and artifact commands.

Use `EditItems([Edit(entity, id, from, to)])` from `oculus-ui` for existing
content. Entity is `cheat`, `question`, `quiz`, `artifact` or `topic`. The
`from` object contains the exact current values of the fields being changed;
`to` contains their replacements. Include `topic_ids` alongside a title,
body or other ordinary field when appropriate. A request to organise a
subject can be one batch of these edits; there is no separate sorting action.
Use only ids and values read from the CLI, and leave ambiguous entries in
Unsorted rather than guessing. The following illustrates the shape after
reading the real entry and topic ids:

```openui
root = Stack([EditItems([Edit("cheat", 17, {title:"Product rule",topic_ids:[1]}, {title:"Product rule",topic_ids:[4,6]})])])
```

Topic metadata also uses `EditItems`, with `Edit("topic", id, from, to)` for
`name`, `description` or `position`. Unsorted is a protected fallback, not a
bucket to rename. Do not change a question's status merely to sort it, reset
its scheduling fields, or activate an agent draft automatically. The student
can accept or reject the edit card; direct CLI update commands belong to the
student's terminal. Deletion is performed by the student in Revision.
