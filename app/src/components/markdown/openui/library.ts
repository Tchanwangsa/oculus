import { List } from "./components/layout/List";
import { createLibrary } from "@openuidev/react-lang";
import { TaskList } from "./components/LiveTasks";
import { Deadline, Upcoming } from "./components/LiveCalendar";
import { Lecture, StudioVideo } from "./components/LiveMedia";
import { CheatNote, ReviewDue } from "./components/LiveRevision";
import { Ask } from "./components/Ask";
import { Chart } from "./components/Chart";
import { Bar } from "./components/charts/Bar";
import { Distribution } from "./components/charts/Distribution";
import { Donut } from "./components/charts/Donut";
import { Histogram } from "./components/charts/Histogram";
import { Line } from "./components/charts/Line";
import { Scatter } from "./components/charts/Scatter";
import { Series } from "./components/charts/Series";
import { Badge } from "./components/layout/Badge";
import { Button } from "./components/layout/Button";
import { Callout } from "./components/layout/Callout";
import { Columns } from "./components/layout/Columns";
import { Input } from "./components/layout/Input";
import { Panel } from "./components/layout/Panel";
import { Progress } from "./components/layout/Progress";
import { Select } from "./components/layout/Select";
import { Stat } from "./components/layout/Stat";
import { Table } from "./components/layout/Table";
import { Tab, Tabs } from "./components/layout/Tabs";
import { Toggle } from "./components/layout/Toggle";
import { Explorer, Slider, Value } from "./components/Explorer";
import { Card, Flashcards } from "./components/Flashcards";
import { PageRef } from "./components/PageRef";
import { Option, Question, Quiz } from "./components/Quiz";
import { Stack } from "./components/Stack";
import { Step, Steps } from "./components/Steps";
import { Text } from "./components/Text";
import { functions } from "./functions";
import { proposalComponents, jobComponents } from "./proposals/definitions";

/**
 * The components an agent may use in an ```openui fence: study widgets,
 * an explain kit (charts and a small layout kit) and proposal cards. Each
 * draws in the app's tokens, so the model picks what to show and the app
 * decides how it looks. Groups and their notes become sections of the
 * generated prompt (`prompt.ts`), which every thread may read, so each
 * component and note earns its length. `Chart` is registered for old
 * programs only; `prompt.ts` hides it.
 */
export const library = createLibrary({
  root: "Stack",
  components: [TaskList, Deadline, Upcoming, Lecture, StudioVideo, CheatNote, ReviewDue, Stack, Text, Flashcards, Card, Quiz, Question, Option, Steps, Step, Explorer, Slider, Value, Chart, Bar, Histogram, Line, Scatter, Series, Donut, Distribution, Panel, Columns, Tabs, Tab, Table, List, Stat, Progress, Badge, Callout, Button, Toggle, Select, Input, PageRef, Ask, ...proposalComponents, ...jobComponents],
  functions,
  componentGroups: [
    { name: "Root", components: ["Stack", "Text"] },
    {
      name: "Recall",
      components: ["Flashcards", "Card", "Quiz", "Question", "Option"],
      notes: [
        "- A standalone Question reveals its explanation immediately after choosing. Only scored Quiz questions require exactly one correct option, marked Option(text, true).",
        "- 5 to 12 cards or 3 to 8 questions; every one checks a single idea.",
      ],
    },
    { name: "Working", components: ["Steps", "Step"] },
    {
      name: "Explore",
      components: ["Explorer", "Slider", "Value"],
      notes: [
        "- A Slider's first argument is a $variable declared with a literal default; Values and charts compute from those $variables.",
        "- There is no ^ operator: write @Pow(base, exponent).",
      ],
    },
    {
      name: "Charts",
      components: ["Bar", "Histogram", "Line", "Scatter", "Series", "Donut", "Distribution"],
      notes: [
        "- Pick by the question: categories → Bar, a spread of values → Histogram, change over x → Line, two measures → Scatter, parts of a whole → Donut, a probability as an area → Distribution. One number is a Stat, not a chart.",
        "- The title names what is plotted; units go in axis labels, e.g. \"Wait (min)\". Data is plain arrays: @Each(@Range(...), \"t\", {x: t, y: ...}) builds points.",
      ],
    },
    {
      name: "Layout",
      components: ["Panel", "Columns", "Tabs", "Tab", "Table", "List", "Stat", "Progress", "Badge", "Callout", "Button", "Toggle", "Select", "Input"],
      notes: [
        "- Block is Text, a chart, Stat, Table, List, Progress, Badge, Callout, Button, Toggle, Select, Input, Slider or Value.",
        "- Panel is the layout card (Card is a flashcard). Columns wraps side by side. Consecutive Stats or pills share rows; List supplies short reading-order items. Blocks directly in Stack share a card when needed.",
        "- Toggle, Select and Input bind a $variable declared with a literal default, like Slider.",
      ],
    },
    { name: "Live", components: ["TaskList", "Deadline", "Upcoming", "Lecture", "StudioVideo", "CheatNote", "ReviewDue"] },
    {
      name: "Data", components: [], notes: [
        '- Live read-only rows: Use Query for live student data, with array defaults: rows = Query("tasks", {project:12}, []). Inspect CLI JSON and IDs first. Never Mutation; writes go through proposals.',
        '- Tools: "tasks" (oculus task list --json), "projects" (project list), "calendar" / "upcoming" (calendar), "deadlines" (calendar --due), "lectures" (lecture list), "revision due" (quiz list --due), "cheat notes" (cheat list), "subjects" (list). Use array defaults; reads refresh after edits; failures offer Retry.',
        '- Distribution functions return names, not densities. Keep a fixed seed while a Slider changes sample size. Use @Bins(@Means(@Exponential(), {}, $n, 400, 42), 20) for a CLT histogram.',
        '- All tools accept limit (default 100, capped at 400). tasks: project, unfiled, column (needs project), due_before (ISO), tag. projects: subject, personal, archived. calendar/upcoming/deadlines: subject, days (0–365, default 14), past, due. lectures: subject. revision due/cheat notes: subject, topic. subjects: limit only. No other args.',
        '- subject is an id or code; omit for all. A single-subject read requires an unambiguous code; calendar also accepts a code prefix spanning terms. Revision also accepts "general". Local data refreshes after edits; @Run(rows) reads again. Query args may depend on $variables.',
      ],
    },
    { name: "Links", components: ["PageRef", "Ask"] },
    { name: "Create", components: ["NewTasks", "NewProject", "Task", "Under", "NewEvents", "Event", "NewNote", "NewCheatNotes", "CheatEntry"], notes: ["- Use one proposal per openui fence. Nothing is written until the student clicks Create.", "- Look up existing ids in the CLI; never guess. Optional arguments may be null to skip a middle argument.", "- amends is the app's pending card id from Cards in this thread, always the last argument."] },
    { name: "Change", components: ["Change", "TaskChanges", "ProjectChanges", "EventChanges", "TagChanges", "NewTag", "DeleteTag", "DeleteTasks", "DeleteEvents", "EditItems", "Edit"], notes: ["- Change(id, from, to) uses the same keys in from and to. Null clears a field.", "- Task keys: title, body, due, starts, estimate, tags, files, column, project. Project keys: name, subject, due, starts, brief, tags, event, status. Event keys: title, start, end, kind, subject, notes. Tag keys: name, color.", "- Only local_<id> events may be changed/deleted; Canvas events are read-only."] },
    { name: "Jobs", components: ["Sync", "Reparse", "Transcribe", "LectureChapters", "LectureEnd", "Download"], notes: ["- Ask with a job card before starting long, billed or downloaded work. One job per fence. No Undo."] },
  ],
});
