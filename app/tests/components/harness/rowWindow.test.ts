import { describe, expect, test } from "bun:test";
import type { HarnessItem } from "@/lib/harness";
import type { ViewRow } from "@/components/harness/timeline/rows";
import {
  PAGE,
  firstId,
  includeId,
  initialFrom,
  pageEarlier,
  settleFrom,
  windowStart,
} from "@/components/harness/timeline/rowWindow";

const item = (id: number): HarnessItem => ({ id, kind: "assistant" }) as HarnessItem;
const row = (id: number): ViewRow => ({ kind: "item", item: item(id), dim: false });
const bundle = (...ids: number[]): ViewRow => ({ kind: "bundle", id: `b-${ids[0]}`, items: ids.map(item) });

/** `n` single-item rows with ids 1..n. */
const rows = (n: number) => Array.from({ length: n }, (_, i) => row(i + 1));

describe("timeline row window", () => {
  test("opens on the newest page", () => {
    const r = rows(100);
    const from = initialFrom(r);
    expect(from).toBe(100 - PAGE + 1);
    expect(windowStart(r, from)).toBe(100 - PAGE);
    expect(r.length - windowStart(r, from)).toBe(PAGE);
  });

  test("a thread of a page or less mounts everything", () => {
    expect(initialFrom(rows(PAGE))).toBeNull();
    expect(initialFrom(rows(3))).toBeNull();
    expect(windowStart(rows(PAGE), null)).toBe(0);
    expect(settleFrom([], undefined)).toBeUndefined();
  });

  test("each load reaches a page further back, then everything", () => {
    const r = rows(100);
    const once = pageEarlier(r, initialFrom(r));
    expect(windowStart(r, once)).toBe(100 - 2 * PAGE);
    const twice = pageEarlier(r, once);
    expect(windowStart(r, twice)).toBe(100 - 3 * PAGE);
    expect(pageEarlier(r, twice)).toBeNull();
  });

  test("appended rows leave the boundary where it was", () => {
    const r = rows(100);
    const from = initialFrom(r);
    const grown = [...r, row(101), row(-5)];
    expect(settleFrom(grown, from)).toBe(from);
    expect(windowStart(grown, from)).toBe(100 - PAGE);
  });

  test("a jump into a bundle mounts the whole bundle", () => {
    const r = [...rows(10), bundle(11, 12, 13), ...rows(60).map((x) => row(firstId(x) + 13))];
    const from = initialFrom(r);
    const to = includeId(r, from, 12);
    expect(to).toBe(11);
    expect(windowStart(r, to)).toBe(10);
  });

  test("a jump never moves the boundary later", () => {
    const r = rows(100);
    const from = initialFrom(r);
    expect(includeId(r, from, 95)).toBe(from);
    expect(includeId(r, from, 1)).toBeNull();
    expect(includeId(r, from, 999)).toBe(from);
    expect(includeId(r, null, 5)).toBeNull();
  });

  test("a boundary a rewind took re-opens on the newest page", () => {
    const r = rows(100);
    const from = pageEarlier(r, initialFrom(r));
    const rewound = r.slice(0, 20);
    expect(settleFrom(rewound, from)).toBeNull();
    const longer = r.slice(0, 35);
    expect(settleFrom(longer, from)).toBe(35 - PAGE + 1);
    expect(windowStart(rewound, from)).toBe(0);
  });
});
