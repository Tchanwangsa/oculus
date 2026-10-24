export type RevisionScope = "all" | "general" | number;
export type RevisionTopicScope = "all" | "unsorted" | number;
export interface RevisionTopic {
  id: number;
  subject_id: number | null;
  name: string;
  description: string;
  position: number;
  is_unsorted: number;
  created_at: string;
  updated_at: string;
}
export type RevisionEntity = "note" | "item" | "set" | "artifact";
export type ItemKind = "card" | "choice" | "numeric" | "short" | "structured";
export type ItemLevel = "easy" | "medium" | "hard";
export type ItemStatus = "draft" | "active" | "suspended";
export interface RevisionNote {
  topic_ids: number[];
  id: number;
  subject_id: number | null;
  title: string;
  body: string;
  tags: string[];
  position: number;
  source_kind: "chat" | "file" | "lecture" | "agent" | "manual";
  source_ref: string | null;
  created_at: string;
  updated_at: string;
}
export interface RevisionItem {
  topic_ids: number[];
  id: number;
  subject_id: number | null;
  kind: ItemKind;
  prompt: string;
  answer: string;
  options: string[];
  tolerance: number | null;
  rubric: { point: string; marks: number }[];
  level: ItemLevel;
  topic: string[];
  style_ref: string | null;
  status: ItemStatus;
  created_by: "agent" | "user";
  due: string;
  stability: number;
  fsrs_difficulty: number;
  reps: number;
  lapses: number;
  state: number;
  learning_steps: number;
  last_review: string | null;
  created_at: string;
  updated_at: string;
}
export interface RevisionSet {
  topic_ids: number[];
  id: number;
  subject_id: number | null;
  title: string;
  description: string;
  created_at: string;
  updated_at: string;
}
export interface RevisionArtifact {
  topic_ids: number[];
  id: number;
  subject_id: number | null;
  title: string;
  program: string;
  state: Record<string, unknown>;
  source_ref: string | null;
  created_at: string;
  updated_at: string;
}
export interface RevisionReview {
  id: number;
  item_id: number;
  set_id: number | null;
  session_id: string | null;
  scheduled: number;
  at: string;
  answer: string;
  rating: number;
  score: number | null;
  max: number | null;
  feedback: string | null;
  judge_model: string | null;
  overridden: number;
  seconds: number | null;
  working: string | null;
}
export type NoteInput = Pick<RevisionNote, "title" | "body"> &
  Partial<
    Pick<
      RevisionNote,
      | "topic_ids"
      | "subject_id"
      | "tags"
      | "source_kind"
      | "source_ref"
      | "position"
    >
  >;
export type ItemInput = Pick<RevisionItem, "kind" | "prompt" | "answer"> &
  Partial<
    Pick<
      RevisionItem,
      | "topic_ids"
      | "subject_id"
      | "options"
      | "tolerance"
      | "rubric"
      | "level"
      | "topic"
      | "style_ref"
      | "status"
      | "created_by"
    >
  >;
export type ArtifactInput = Pick<RevisionArtifact, "title" | "program"> &
  Partial<
    Pick<RevisionArtifact, "topic_ids" | "subject_id" | "state" | "source_ref">
  >;
