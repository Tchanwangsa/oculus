import { expect, test } from "bun:test";
import {
  inheritedTopic,
  matchesTopic,
  topicsForSubject,
} from "@/lib/revision/topics";
import type { RevisionTopic } from "@/lib/revision/types";
const topics = [
  { id: 1, subject_id: null, name: "Unsorted", is_unsorted: 1 },
  { id: 2, subject_id: 10, name: "Unsorted", is_unsorted: 1 },
  { id: 3, subject_id: 10, name: "Differentiation", is_unsorted: 0 },
  { id: 4, subject_id: 20, name: "Cells", is_unsorted: 0 },
] as RevisionTopic[];
test("topic scope preserves cross-topic membership with one entity", () => {
  const row = { topic_ids: [3, 4] };
  expect(matchesTopic(row, topics, 3)).toBe(true);
  expect(matchesTopic(row, topics, 4)).toBe(true);
  expect(matchesTopic(row, topics, "all")).toBe(true);
  expect(matchesTopic(row, topics, "unsorted")).toBe(false);
});
test("Unsorted aggregates each subject's fallback", () => {
  expect(matchesTopic({ topic_ids: [1] }, topics, "unsorted")).toBe(true);
  expect(matchesTopic({ topic_ids: [2] }, topics, "unsorted")).toBe(true);
  expect(topicsForSubject(topics, "general").map((t) => t.id)).toEqual([1]);
  expect(topicsForSubject(topics, 10).map((t) => t.id)).toEqual([2, 3]);
});
test("creation inherits a selected topic's subject only when compatible", () => {
  expect(inheritedTopic(topics, "all", 3)?.subject_id).toBe(10);
  expect(inheritedTopic(topics, 10, 3)?.id).toBe(3);
  expect(inheritedTopic(topics, 20, 3)).toBeUndefined();
  expect(inheritedTopic(topics, "general", 3)).toBeUndefined();
  expect(inheritedTopic(topics, 10, "unsorted")).toBeUndefined();
});
