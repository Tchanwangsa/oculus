import { expect, test } from "bun:test";
import { deriveBehind } from "@/hooks/data/overview/behind";
import { deriveProgress } from "@/hooks/data/overview/progress";
import { classes, date, first, lecture, strip } from "./fixtures";

test("behind excludes untimetabled, watched, future and other-subject recordings", () => {
  const recordings = [lecture("2026-10-05T10:05:00", 1, "behind"), lecture("2026-10-06T10:00:00"),
    { ...lecture("2026-10-05T10:00:00", 1, "end"), content_end_status: "ready", content_end_seconds: 600, progress_seconds: 590 },
    { ...lecture("2026-10-05T10:00:00", 1, "done"), completed: 1 },
    lecture("2026-10-19T10:00:00"), lecture("2026-10-05T10:00:00", 2), lecture("bad")];
  expect(deriveBehind(1, strip, recordings, classes, date()).lectures.map((row) => row.id)).toEqual(["behind"]);
  expect(deriveBehind(1, strip, recordings, [], date()).count).toBe(2);
  expect(deriveBehind(1, strip, recordings, [], date(1)).count).toBe(0);
});

test("behind stays quiet in breaks, after teaching and without a strip", () => {
  const recordings = [lecture("2026-10-05T10:05:00")];
  for (const current of [1, 3, 5, -1]) expect(deriveBehind(1, { ...strip, current }, recordings, classes, date(30)).count).toBe(0);
  expect(deriveBehind(1, { ...strip, weeks: [], current: -1 }, recordings, [], date()).lectures).toEqual([]);
  const later = deriveBehind(1, { ...strip, current: 2 }, recordings, classes, date(20));
  expect(later.count).toBe(1);
});

test("progress counts watched over total per teaching week, including other streams", () => {
  const recordings = [lecture("2026-10-05T10:05:00"),
    { ...lecture("2026-10-06T10:00:00"), progress_seconds: 3580 },
    lecture("2026-10-13T10:00:00"), lecture("bad"), lecture("2026-10-05T10:00:00", 2)];
  const result = deriveProgress(1, strip, recordings);
  expect(result).toEqual([{ weekKey: first, watched: 1, total: 2 }, { weekKey: strip.weeks[2].key, watched: 0, total: 0 }]);
  expect(deriveProgress(1, { ...strip, weeks: [], current: -1 }, recordings)).toEqual([]);
});
