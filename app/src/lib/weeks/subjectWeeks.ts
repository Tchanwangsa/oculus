import { addDays } from "@/lib/planning/calendar/dates";
import { mondayFromKey, weekKey, weekOf } from "./dates";
import { termTimetable } from "./timetable";
import type { SubjectWeeks, SubjectWeeksInput, Week } from "./types";

export function subjectWeeks(input: SubjectWeeksInput): SubjectWeeks {
  const { subjectId, subjects, classes, lectures, now } = input;
  const subject = subjects.find((row) => row.id === subjectId);
  const termIds = new Set(subjects
    .filter((row) => subject && row.term_name === subject.term_name)
    .map((row) => row.id));
  const timetable = termTimetable(classes.filter((row) => termIds.has(row.subject_id)));
  const source = timetable.subjectWeeks.size > 0 ? "term" : "recordings";
  let teaching: Set<number>;
  let last: number | undefined;

  if (source === "term") {
    teaching = timetable.teaching;
    const ownWeeks = timetable.subjectWeeks.get(subjectId);
    // Sparse outliers cannot extend a subject beyond qualifying teaching weeks.
    const termTeaching = [...teaching];
    const ownTeaching = termTeaching.filter((key) => ownWeeks?.has(key));
    last = ownTeaching.length > 0 ? ownTeaching[ownTeaching.length - 1] : termTeaching[termTeaching.length - 1];
  } else {
    const recorded = lectures
      .filter((row) => row.subject_id === subjectId)
      .map((row) => weekKey(new Date(row.date)))
      .filter(Number.isFinite)
      .sort((a, b) => a - b);
    teaching = new Set(recorded);
    last = recorded[recorded.length - 1];
  }

  const first = teaching.values().next().value;
  const strip: SubjectWeeks = { weeks: [], current: -1, source };
  if (first === undefined || last === undefined) return strip;

  let number = 0;
  for (let monday = mondayFromKey(first); weekKey(monday) <= last; monday = addDays(monday, 7)) {
    const key = weekKey(monday);
    const isTeaching = source === "recordings" || teaching.has(key);
    if (isTeaching) number++;
    strip.weeks.push({
      key, kind: isTeaching ? "teaching" : "break",
      label: isTeaching ? String(number) : "Break",
      number: isTeaching ? number : null,
      start: monday, end: addDays(monday, 7),
    });
  }
  for (let after = 1; after <= 3; after++) {
    const start = addDays(mondayFromKey(last), after * 7);
    const week: Week = {
      key: weekKey(start), kind: "after", label: `+${after}`, number: null,
      start, end: addDays(start, 7),
    };
    strip.weeks.push(week);
  }
  const currentKey = weekOf(strip, now);
  strip.current = strip.weeks.findIndex((week) => week.key === currentKey);
  return strip;
}
