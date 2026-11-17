import { expect, test } from "bun:test";
import { deriveEvents } from "@/hooks/data/overview/events";
import { classes, date, event, first, iso, lecture, strip } from "./fixtures";

test("events prefer the subject's entire Canvas class layer, preserving locations and ends", () => {
  const rows = [event(iso(6), 1, { end_at: iso(6, 11), location: "Room 2" }), ...classes];
  const items = deriveEvents(1, strip, first, rows, [lecture(iso(6))], date(6, 10));
  expect(items.map((item) => item.kind)).toEqual(["class", "class"]);
  expect(items.map((item) => item.past)).toEqual([true, false]);
  expect(items[1].location).toBe("Room 2");
  expect(items[1].end).toEqual(date(6, 11));
  expect(deriveEvents(1, strip, strip.weeks[1].key, classes, [lecture(iso(13))], date())).toEqual([]);
});

test("events fall back to recordings only without usable own Canvas classes", () => {
  const recordings = [lecture("2026-10-06T10:00:00"), lecture("bad"), lecture(iso(6), 2)];
  const items = deriveEvents(1, strip, first, [event(iso(), 2), event(iso(), 1, { start_at: "bad" })], recordings, date(6, 12));
  expect(items).toHaveLength(1);
  expect(items[0].kind).toBe("recording");
  expect(items[0].end).toEqual(date(6, 11));
  expect(items[0].past).toBe(true);
  expect(deriveEvents(1, { ...strip, weeks: [], current: -1 }, null, [], recordings, date())).toEqual([]);
});
