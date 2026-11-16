import type { Subject } from "@/lib/db";
import { displayCode, displayName } from "@/lib/format/format";
import type { SubjectGroup } from "@/lib/ui/subjectGroups";
import { completedKey } from "@/stores/chat/completedStore";

/** The Subject view's groups plus an empty one for each subject (or
 *  General) whose threads are all completed, so its fold is still reachable. */
export function withCompletedGroups<T>(
  groups: SubjectGroup<T>[],
  subjects: Subject[],
  counts: Record<string, number>,
): SubjectGroup<T>[] {
  const have = new Set(groups.map((g) => g.key));
  const extra: SubjectGroup<T>[] = [];
  for (const s of subjects) {
    const key = completedKey(s.id);
    if (counts[key] && !have.has(key)) {
      extra.push({ key, label: displayCode(s.code), title: displayName(s.name, s.code), subjectId: s.id, items: [] });
    }
  }
  const general = completedKey(null);
  if (counts[general] && !have.has(general)) {
    extra.push({ key: general, label: "General", title: "Not scoped to a subject", subjectId: null, items: [] });
  }
  return extra.length ? [...groups, ...extra] : groups;
}
