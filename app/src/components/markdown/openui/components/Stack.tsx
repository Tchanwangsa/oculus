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
import { Quiz } from "./Quiz";
import { Steps } from "./Steps";
import { Text } from "./Text";
import { WidgetIndex } from "../state";
import { Task, NewCheatNotes, NewTasks, NewProject, NewEvents, NewNote, EditItems, TaskChanges, ProjectChanges, EventChanges, TagChanges, DeleteTasks, DeleteEvents, Sync, Reparse, Transcribe, LectureChapters, LectureEnd, Download } from "../proposals/definitions";

/** The program's root: widgets top to bottom, with Ask buttons and PageRefs
 *  gathered onto one wrapping row so they read as a footer, not a list. */
export const Stack = defineComponent({
  name: "Stack",
  description: "The root: study widgets top to bottom. Ask and PageRef items line up in a row.",
  props: z.object({
    items: z.array(z.union([Task.ref, TaskList.ref, Deadline.ref, Upcoming.ref, Lecture.ref, StudioVideo.ref, CheatNote.ref, ReviewDue.ref, NewCheatNotes.ref, EditItems.ref, Flashcards.ref, Quiz.ref, Steps.ref, Explorer.ref, Text.ref, PageRef.ref, Ask.ref, NewTasks.ref, NewProject.ref, NewEvents.ref, NewNote.ref, TaskChanges.ref, ProjectChanges.ref, EventChanges.ref, TagChanges.ref, DeleteTasks.ref, DeleteEvents.ref, Sync.ref, Reparse.ref, Transcribe.ref, LectureChapters.ref, LectureEnd.ref, Download.ref])),
  }),
  component: function StackView({ props, renderNode }) {
    const items = Array.isArray(props.items) ? props.items : [];
    const inline = (n: unknown) => {
      const t = (n as { typeName?: string } | null)?.typeName;
      return t === "Ask" || t === "PageRef";
    };
    const blocks = items.map((node, i) => !inline(node) && (
      <WidgetIndex.Provider key={i} value={i}>{renderNode(node)}</WidgetIndex.Provider>
    ));
    const chips = items.filter(inline);
    return (
      <div className="flex flex-col">
        {blocks}
        {chips.length > 0 && <div className="my-2 flex flex-wrap items-center gap-2">{renderNode(chips)}</div>}
      </div>
    );
  },
});
