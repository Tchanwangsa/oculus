import type { Subject } from "../db";
import { displayCode, displayName } from "../format/format";

export interface SubjectGroup<T> {
  key: string;
  label: string;
  title: string;
  subjectId: number | null;
  items: T[];
}

/** Preserve first-appearance and row order; missing subjects join the unscoped group. */
export function groupBySubject<T extends { subject_id: number | null }>(
  items: T[],
  subjects: Subject[],
  unscoped: { key: string; label: string },
): SubjectGroup<T>[] {
  const bySubject = new Map(subjects.map((s) => [s.id, s]));
  const groups = new Map<string, SubjectGroup<T>>();
  for (const item of items) {
    const subject = item.subject_id == null ? undefined : bySubject.get(item.subject_id);
    const key = subject ? String(subject.id) : unscoped.key;
    let group = groups.get(key);
    if (!group) {
      group = {
        key,
        label: subject ? displayCode(subject.code) : unscoped.label,
        title: subject ? displayName(subject.name, subject.code) : "Not scoped to a subject",
        subjectId: subject?.id ?? null,
        items: [],
      };
      groups.set(key, group);
    }
    group.items.push(item);
  }
  return [...groups.values()];
}
