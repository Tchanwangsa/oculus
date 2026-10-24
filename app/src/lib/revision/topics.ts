import type { RevisionScope, RevisionTopic, RevisionTopicScope } from "./types";

export function topicsForSubject(
  topics: RevisionTopic[],
  scope: RevisionScope,
) {
  return topics.filter(
    (topic) =>
      scope === "all" ||
      topic.subject_id === (scope === "general" ? null : scope),
  );
}
export function matchesTopic(
  row: { topic_ids: number[] },
  topics: RevisionTopic[],
  scope: RevisionTopicScope,
) {
  if (scope === "all") return true;
  if (scope === "unsorted")
    return topics.some(
      (topic) => !!topic.is_unsorted && row.topic_ids.includes(topic.id),
    );
  return row.topic_ids.includes(scope);
}
export function inheritedTopic(
  topics: RevisionTopic[],
  scope: RevisionScope,
  selected: RevisionTopicScope,
) {
  const topic =
    typeof selected === "number"
      ? topics.find((topic) => topic.id === selected)
      : undefined;
  return topic &&
    (scope === "all" ||
      topic.subject_id === (scope === "general" ? null : scope))
    ? topic
    : undefined;
}
