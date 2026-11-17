import type { DbCalendarEvent, Lecture, Subject } from "@/lib/db";

export type WeekKind = "teaching" | "break" | "after";

export interface Week {
  /** UTC-encoded civil Monday, stable across local DST offsets. */
  key: number;
  kind: WeekKind;
  label: string;
  number: number | null;
  start: Date;
  /** Exclusive: the next local Monday. */
  end: Date;
}

export interface SubjectWeeks {
  weeks: Week[];
  /** Index into weeks; -1 when no span can be derived. */
  current: number;
  source: "term" | "recordings";
}

export interface SubjectWeeksInput {
  subjectId: number;
  subjects: Subject[];
  classes: DbCalendarEvent[];
  lectures: Lecture[];
  now: Date;
}

export type RecordingStatus = "timetabled" | "untimetabled" | "unknown";
