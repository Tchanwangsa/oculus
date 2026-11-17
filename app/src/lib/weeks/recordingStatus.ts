import type { DbCalendarEvent, Lecture } from "@/lib/db";
import { timedClasses } from "./timetable";
import type { RecordingStatus } from "./types";

/** Recordings start a few minutes either side of their class. */
const MATCH_WINDOW_MINUTES = 30;

export function recordingStatus(lecture: Lecture, classes: DbCalendarEvent[]): RecordingStatus {
  const own = timedClasses(classes).filter((event) => event.subject_id === lecture.subject_id);
  if (own.length === 0) return "unknown";
  const recording = new Date(lecture.date);
  const matches = own.some((event) => {
    const start = new Date(event.start_at);
    const minutes = (start.getTime() - recording.getTime()) / 60_000;
    return start.getFullYear() === recording.getFullYear()
      && start.getMonth() === recording.getMonth()
      && start.getDate() === recording.getDate()
      && Math.abs(minutes) <= MATCH_WINDOW_MINUTES;
  });
  return matches ? "timetabled" : "untimetabled";
}
