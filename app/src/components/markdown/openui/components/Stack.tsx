import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { TaskList } from "./LiveTasks";
import { Deadline, Upcoming } from "./LiveCalendar";
import { Lecture, StudioVideo } from "./LiveMedia";
import { CheatNote, ReviewDue } from "./LiveRevision";
import { Ask } from "./Ask";
import { Explorer } from "./Explorer";
import { Flashcards } from "./Flashcards";
import { PageRef } from "./PageRef";
import { Quiz, Question } from "./Quiz";
import { Steps } from "./Steps";
import { Block } from "./layout/Block";
import { Columns } from "./layout/Columns";
import { Items } from "./layout/items";
import { Panel } from "./layout/Panel";
import { Tabs } from "./layout/Tabs";
import { Task, NewCheatNotes, NewTasks, NewProject, NewEvents, NewNote, EditItems, TaskChanges, ProjectChanges, EventChanges, TagChanges, DeleteTasks, DeleteEvents, Sync, Reparse, Transcribe, LectureChapters, LectureEnd, Download } from "../proposals/definitions";

const footer = (n: unknown) => {
  const t = (n as { typeName?: string } | null)?.typeName;
  return t === "Ask" || t === "PageRef";
};

/** The program's root: blocks top to bottom (grouped by `Items`), with Ask
 *  buttons and PageRefs gathered onto one wrapping row so they read as a
 *  footer, not a list. */
export const Stack = defineComponent({
  name: "Stack",
  description: "The root: items top to bottom. Ask and PageRef items line up in a row.",
  props: z.object({
    items: z.array(z.union([Task.ref, TaskList.ref, Deadline.ref, Upcoming.ref, Lecture.ref, StudioVideo.ref, CheatNote.ref, ReviewDue.ref, NewCheatNotes.ref, EditItems.ref, Flashcards.ref, Quiz.ref, Question.ref, Steps.ref, Explorer.ref, Block, Panel.ref, Columns.ref, Tabs.ref, PageRef.ref, Ask.ref, NewTasks.ref, NewProject.ref, NewEvents.ref, NewNote.ref, TaskChanges.ref, ProjectChanges.ref, EventChanges.ref, TagChanges.ref, DeleteTasks.ref, DeleteEvents.ref, Sync.ref, Reparse.ref, Transcribe.ref, LectureChapters.ref, LectureEnd.ref, Download.ref])),
  }),
  component: function StackView({ props, renderNode }) {
    const items = Array.isArray(props.items) ? props.items : [];
    const chips = items.filter(footer);
    return (
      <div className="flex flex-col">
        <Items items={items.map((n) => (footer(n) ? null : n))} renderNode={renderNode} />
        {chips.length > 0 && <div className="my-2 flex flex-wrap items-center gap-2">{renderNode(chips)}</div>}
      </div>
    );
  },
});
