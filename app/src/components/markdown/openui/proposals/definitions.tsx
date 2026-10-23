import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { ProposalCard } from "./ProposalCard";
import { JobCard } from "./JobCard";

const text = () => z.string().nullish();
const list = () => z.array(z.string()).nullish();
const amends = () => text().describe("The app's id of a pending card to replace, if any");
const id = () => z.number().int().positive();
const silent = () => null;
const nestedTask: z.ZodType = z.lazy((): z.ZodType => Task.ref as unknown as z.ZodType);

export const Task = defineComponent({
  name: "Task", description: "A new task; subtasks may nest only one level. Used only inside a creation card.",
  props: z.object({
    title: z.string(), due: text(), estimate: z.number().int().nonnegative().nullish(), column: text(),
    subtasks: z.array(nestedTask).nullish(), tags: list(), body: text(), starts: text(), files: list(),
  }), component: silent,
});
export const Under = defineComponent({
  name: "Under", description: "New subtasks grouped under an existing task; only in NewTasks.",
  props: z.object({ task: id(), subtasks: z.array(Task.ref) }), component: silent,
});

export const NewTasks = defineComponent({
  name: "NewTasks", description: "Propose creating tasks in an existing project, or null for unfiled. The student edits and confirms.",
  props: z.object({ project: id().nullish(), items: z.array(z.union([Task.ref, Under.ref])), amends: amends() }),
  component: ({ props }) => <ProposalCard kind="NewTasks" input={props} />,
});
export const NewProject = defineComponent({
  name: "NewProject", description: "Propose a new project and its tasks together. Nothing is written until Create.",
  props: z.object({ name: z.string(), subject: z.string().nullish(), tasks: z.array(Task.ref), due: text(), brief: text(), starts: text(), tags: list(), event: text(), amends: amends() }),
  component: ({ props }) => <ProposalCard kind="NewProject" input={props} />,
});
export const Event = defineComponent({
  name: "Event", description: "A proposed calendar event. A study session uses kind class and an end. Other kinds cannot have an end.",
  props: z.object({ title: z.string(), start: z.string(), end: text(), kind: z.enum(["note", "due", "class"]).nullish(), subject: text(), notes: text() }), component: silent,
});
export const NewEvents = defineComponent({
  name: "NewEvents", description: "Propose the student's own calendar events, confirmed together.",
  props: z.object({ events: z.array(Event.ref), amends: amends() }),
  component: ({ props }) => <ProposalCard kind="NewEvents" input={props} />,
});
export const NewNote = defineComponent({
  name: "NewNote", description: "Propose a note in a subject, with its complete markdown body.",
  props: z.object({ subject: z.string(), title: z.string(), body: z.string(), amends: amends() }),
  component: ({ props }) => <ProposalCard kind="NewNote" input={props} />,
});

export const Change = defineComponent({
  name: "Change", description: "An existing id, the exact fields read from it, and replacement values for those same fields. Null clears a field.",
  props: z.object({ id: z.union([id(), z.string()]), from: z.record(z.string(), z.unknown()), to: z.record(z.string(), z.unknown()) }), component: silent,
});
export const NewTag = defineComponent({
  name: "NewTag", description: "A proposed shared tag and its palette colour key.",
  props: z.object({ name: z.string(), color: z.enum(["slate", "indigo", "green", "amber", "rose", "purple", "teal", "orange"]) }), component: silent,
});
export const DeleteTag = defineComponent({
  name: "DeleteTag", description: "Remove a shared tag by id, including its links on tasks and projects.",
  props: z.object({ tag: id() }), component: silent,
});
export const TaskChanges = defineComponent({
  name: "TaskChanges", description: "Propose task field changes, moves or refile. Each Change carries matching from and to keys.",
  props: z.object({ changes: z.array(Change.ref), amends: amends() }),
  component: ({ props }) => <ProposalCard kind="TaskChanges" input={props} />,
});
export const ProjectChanges = defineComponent({
  name: "ProjectChanges", description: "Propose changes to projects, including archiving.",
  props: z.object({ changes: z.array(Change.ref), amends: amends() }),
  component: ({ props }) => <ProposalCard kind="ProjectChanges" input={props} />,
});
export const EventChanges = defineComponent({
  name: "EventChanges", description: "Propose changes only to the student's local_<id> events; Canvas events are read-only.",
  props: z.object({ changes: z.array(Change.ref), amends: amends() }),
  component: ({ props }) => <ProposalCard kind="EventChanges" input={props} />,
});
export const TagChanges = defineComponent({
  name: "TagChanges", description: "Propose shared tag changes, creations or removals.",
  props: z.object({ changes: z.array(z.union([Change.ref, NewTag.ref, DeleteTag.ref])), amends: amends() }),
  component: ({ props }) => <ProposalCard kind="TagChanges" input={props} />,
});
export const DeleteTasks = defineComponent({
  name: "DeleteTasks", description: "Propose deleting tasks and their subtasks; Undo restores them if nothing has changed.",
  props: z.object({ tasks: z.array(id()), amends: amends() }),
  component: ({ props }) => <ProposalCard kind="DeleteTasks" input={props} />,
});
export const DeleteEvents = defineComponent({
  name: "DeleteEvents", description: "Propose deleting only the student's local_<id> calendar events.",
  props: z.object({ events: z.array(z.string()), amends: amends() }),
  component: ({ props }) => <ProposalCard kind="DeleteEvents" input={props} />,
});

export const Sync = defineComponent({ name: "Sync", description: "Ask before syncing subjects and optionally lectures.", props: z.object({ subjects: z.array(z.string()), lectures: z.boolean().nullish() }), component: ({ props }) => <JobCard kind="Sync" input={props} /> });
export const Reparse = defineComponent({ name: "Reparse", description: "Ask before reparsing and embedding the listed PDF library paths.", props: z.object({ files: z.array(z.string()) }), component: ({ props }) => <JobCard kind="Reparse" input={props} /> });
export const Transcribe = defineComponent({ name: "Transcribe", description: "Ask before transcribing a media file using the configured engine.", props: z.object({ file: z.string() }), component: ({ props }) => <JobCard kind="Transcribe" input={props} /> });
export const LectureChapters = defineComponent({ name: "LectureChapters", description: "Ask before finding chapters using the configured job model.", props: z.object({ lecture: z.string() }), component: ({ props }) => <JobCard kind="LectureChapters" input={props} /> });
export const LectureEnd = defineComponent({ name: "LectureEnd", description: "Ask before finding a lecture's content end using the configured model.", props: z.object({ lecture: z.string() }), component: ({ props }) => <JobCard kind="LectureEnd" input={props} /> });
export const Download = defineComponent({ name: "Download", description: "Ask before downloading aligner, voices starter pack or a voice id.", props: z.object({ item: z.string() }), component: ({ props }) => <JobCard kind="Download" input={props} /> });

export const proposalComponents = [Task, Under, NewTasks, NewProject, Event, NewEvents, NewNote, Change, NewTag, DeleteTag, TaskChanges, ProjectChanges, EventChanges, TagChanges, DeleteTasks, DeleteEvents];
export const jobComponents = [Sync, Reparse, Transcribe, LectureChapters, LectureEnd, Download];
