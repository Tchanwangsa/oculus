import type { Subject } from "@/lib/db";

/** Subjects by term name, in the order they come (`getSubjects`: newest term
 *  first). A Map, since an object lists integer-like names ("2025") first, in
 *  ascending order. */
export function groupByTerm(subjects: Subject[]): Map<string, Subject[]> {
  const groups = new Map<string, Subject[]>();
  for (const s of subjects) {
    const key = s.term_name ?? "Unknown term";
    const group = groups.get(key);
    if (group) group.push(s);
    else groups.set(key, [s]);
  }
  return groups;
}
