import { describe, expect, test } from "bun:test";
import { fmtResets, parseErrorMeta, usageLimitHit, type HarnessItem, type RateWindow } from "@/lib/harness";
import { threadBannerItems } from "@/components/harness/composer/ThreadBanners";

let id = 1;
const row = (kind: HarnessItem["kind"], meta: unknown = null, run: number | null = 7): HarnessItem => ({
  id: id++,
  thread_id: 1,
  kind,
  ref_id: null,
  content: "x",
  meta: meta == null ? null : JSON.stringify(meta),
  created_at: "2026-11-15 10:00:00",
  run_id: run,
});
const NOW = 1_800_000_000_000;
const window = (used: number, resetsInMin: number | null): RateWindow => ({
  label: "5h",
  used_percent: used,
  resets_at: resetsInMin == null ? null : (NOW + resetsInMin * 60_000) / 1000,
});

describe("usage limit", () => {
  test("an error row keeps its limit mark", () => {
    expect(parseErrorMeta(row("error", { limit: true }))).toEqual({ limit: true });
    expect(parseErrorMeta(row("error", { auth: "claude", limit: true }))).toEqual({ auth: "claude", limit: true });
    expect(parseErrorMeta(row("error", { limit: "yes" }))).toEqual({});
  });

  test("the latest run ending at a limit is a hit, keyed by its run", () => {
    const items = [row("user"), row("tool"), row("error", { limit: true })];
    expect(usageLimitHit(items, [], NOW)).toEqual({ key: "run:7", resets: null });
  });

  test("a new question clears it", () => {
    expect(usageLimitHit([row("error", { limit: true }), row("user", null, 8)], [], NOW)).toBeNull();
  });

  test("an ordinary failure is not a hit", () => {
    expect(usageLimitHit([row("user"), row("error")], [], NOW)).toBeNull();
  });

  test("the reset is the fullest window's", () => {
    const hit = usageLimitHit([row("user"), row("error", { limit: true })], [window(40, 10), window(100, 125)], NOW);
    expect(hit?.resets).toBe("Resets in 2 hr 5 min");
  });

  test("a reset already past says nothing", () => {
    expect(fmtResets((NOW - 60_000) / 1000, NOW)).toBeNull();
    expect(fmtResets(null, NOW)).toBeNull();
  });
});

describe("threadBannerItems", () => {
  const limited = [row("user"), row("error", { limit: true }, 9)];
  test("a limited run shows the notice, with the reset when known", () => {
    const [b] = threadBannerItems(limited, [window(100, 30)], [], () => {}, NOW);
    expect(b.title).toBe("Usage limit reached");
    expect(b.description).toBe("Resets in 30 min");
    expect(b.id).toBe("limit:run:9");
  });
  test("without a window it says to retry by hand", () => {
    const [b] = threadBannerItems(limited, [], [], () => {}, NOW);
    expect(b.description).toBe("Reset time unavailable; retry manually");
  });
  test("dismissed per run", () => {
    expect(threadBannerItems(limited, [], ["limit:run:9"], () => {}, NOW)).toEqual([]);
    const next = [...limited, row("user", null, 10), row("error", { limit: true }, 10)];
    expect(threadBannerItems(next, [], ["limit:run:9"], () => {}, NOW).map((b) => b.id)).toEqual(["limit:run:10"]);
  });
  test("dismissing names the notice", () => {
    let gone = "";
    threadBannerItems(limited, [], [], (i) => (gone = i), NOW)[0].onDismiss?.();
    expect(gone).toBe("limit:run:9");
  });
});
