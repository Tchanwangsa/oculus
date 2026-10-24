import { createLibrary } from "@openuidev/react-lang";
import { TaskList } from "./components/LiveTasks";
import { Deadline, Upcoming } from "./components/LiveCalendar";
import { Lecture, StudioVideo } from "./components/LiveMedia";
import { CheatNote, ReviewDue } from "./components/LiveRevision";
import { Ask } from "./components/Ask";
import { Chart } from "./components/Chart";
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
 * The components an agent may use in an ```openui fence: a few study widgets,
 * not a layout kit. Every widget draws its own card, so the model picks what
 * to show and the app decides how it looks. Groups and their notes become
 * sections of the generated prompt (`prompt.ts`).
 */
export const library = createLibrary({
  root: "Stack",
  components: [TaskList, Deadline, Upcoming, Lecture, StudioVideo, CheatNote, ReviewDue, Stack, Text, Flashcards, Card, Quiz, Question, Option, Steps, Step, Explorer, Slider, Value, Chart, PageRef, Ask, ...proposalComponents, ...jobComponents],
  functions,
  componentGroups: [
    { name: "Root", components: ["Stack", "Text"] },
    {
      name: "Recall",
      components: ["Flashcards", "Card", "Quiz", "Question", "Option"],
      notes: [
        "- Mark exactly one answer per question with Option(text, true).",
        "- 5 to 12 cards or 3 to 8 questions; every one checks a single idea.",
      ],
    },
    { name: "Working", components: ["Steps", "Step"] },
    {
      name: "Explore",
      components: ["Explorer", "Slider", "Value", "Chart"],
      notes: [
        "- A Slider's first argument is a $variable declared with a literal default; Values and Charts compute from those $variables.",
        "- There is no ^ operator: write @Pow(base, exponent).",
      ],
    },
    { name: "Live", components: ["TaskList", "Deadline", "Upcoming", "Lecture", "StudioVideo", "CheatNote", "ReviewDue"] },
    { name: "Links", components: ["PageRef", "Ask"] },
    { name: "Create", components: ["NewTasks", "NewProject", "Task", "Under", "NewEvents", "Event", "NewNote", "NewCheatNotes", "CheatEntry"], notes: ["- Use one proposal per openui fence. Nothing is written until the student clicks Create.", "- Look up existing ids in the CLI; never guess. Optional arguments may be null to skip a middle argument.", "- amends is the app's pending card id from Cards in this thread, always the last argument."] },
    { name: "Change", components: ["Change", "TaskChanges", "ProjectChanges", "EventChanges", "TagChanges", "NewTag", "DeleteTag", "DeleteTasks", "DeleteEvents", "EditItems", "Edit"], notes: ["- Change(id, from, to) uses the same keys in from and to. Null clears a field.", "- Task keys: title, body, due, starts, estimate, tags, files, column, project. Project keys: name, subject, due, starts, brief, tags, event, status. Event keys: title, start, end, kind, subject, notes. Tag keys: name, color.", "- Only local_<id> events may be changed/deleted; Canvas events are read-only."] },
    { name: "Jobs", components: ["Sync", "Reparse", "Transcribe", "LectureChapters", "LectureEnd", "Download"], notes: ["- Ask with a job card before starting long, billed or downloaded work. One job per fence. No Undo."] },
  ],
});
