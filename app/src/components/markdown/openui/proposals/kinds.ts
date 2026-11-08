import { ArrowsClockwise, CalendarBlank, CheckSquare, DownloadSimple, FileText, FlagCheckered, FolderSimple, ListNumbers, Microphone, Note, PencilSimpleLine, Tag, type Icon } from "@phosphor-icons/react";

/** Each card kind's glyph: the item it writes, or the job it runs. */
export function kindIcon(kind: string): Icon {
  const jobs: Record<string, Icon> = { Sync: ArrowsClockwise, Reparse: FileText, Transcribe: Microphone, LectureChapters: ListNumbers, LectureEnd: FlagCheckered, Download: DownloadSimple };
  if (jobs[kind]) return jobs[kind];
  if (kind === "NewCheatNotes") return Note;
  if (kind === "EditItems") return PencilSimpleLine;
  return kind.includes("Project") ? FolderSimple : kind.includes("Event") ? CalendarBlank : kind.includes("Note") ? FileText : kind.includes("Tag") ? Tag : CheckSquare;
}

const NOUNS: Record<string, string> = {
  NewTasks: "new tasks", NewProject: "a new project", NewEvents: "new events", NewNote: "a new note", NewCheatNotes: "cheat sheet entries",
  TaskChanges: "task changes", ProjectChanges: "project changes", EventChanges: "event changes", TagChanges: "tag changes",
  DeleteTasks: "task deletions", DeleteEvents: "event deletions", EditItems: "edits",
  Sync: "a sync", Reparse: "a re-parse", Transcribe: "a transcription", LectureChapters: "a chapter search", LectureEnd: "a lecture-end search", Download: "a download",
};

/** What a card is about, in words, for the skeleton drawn before its props arrive. */
export const kindNoun = (kind: string) => NOUNS[kind] ?? "a proposal";
