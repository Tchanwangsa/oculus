import { expect, test } from "bun:test";
import {
  DEFAULT_HEIGHT,
  HEIGHT_MESSAGE,
  MIN_HEIGHT,
  RUNAWAY_HEIGHT_CEILING,
  initialPageHeight,
  nextPageHeight,
  prepare,
  readPageHeight,
  type PageHeightReport,
} from "@/components/markdown/embed/pagePrepare";

const report = (height: number, overrides: Partial<PageHeightReport> = {}): PageHeightReport => ({
  height, viewportHeight: DEFAULT_HEIGHT, viewportWidth: 800, revision: 0, ...overrides,
});

function heldPage() {
  let state = initialPageHeight();
  for (let i = 0; i < 4; i++) {
    state = nextPageHeight(state, report(state.height + 16, { viewportHeight: state.height }));
  }
  expect(state.held).toBe(true);
  return state;
}

test("an inline page takes a report above 560 at its full CSS-pixel height", () => {
  const state = nextPageHeight(initialPageHeight(), report(1_240));
  expect(state.height).toBe(1_240);
  expect(nextPageHeight(state, report(1_240))).toBe(state);
  expect(nextPageHeight(state, report(12_000)).height).toBe(12_000);
});

test("the initial height, minimum and rounding still apply", () => {
  expect(initialPageHeight().height).toBe(DEFAULT_HEIGHT);
  expect(nextPageHeight(initialPageHeight(), report(20)).height).toBe(MIN_HEIGHT);
  expect(nextPageHeight(initialPageHeight(), report(120.1)).height).toBe(121);
});

test("viewport plus a constant holds before its third matching resize response", () => {
  const state = heldPage();
  expect(state.height).toBe(368);
  expect(state.height).toBeLessThan(RUNAWAY_HEIGHT_CEILING);
  expect(nextPageHeight(state, report(384, { viewportHeight: 368 }))).toBe(state);
});

test("one-pixel rounding differences and duplicate reports cannot restart feedback", () => {
  let state = initialPageHeight();
  for (const delta of [16, 16, 17, 16]) {
    const message = report(state.height + delta, { viewportHeight: state.height });
    state = nextPageHeight(state, message);
    expect(nextPageHeight(state, message)).toBe(state);
  }
  expect(state.height).toBe(369);
  expect(state.held).toBe(true);
});

test("rows arriving in equal steps are accepted even when they coincide with frame resizes", () => {
  let state = initialPageHeight();
  for (let revision = 1; revision <= 100; revision++) {
    const height = state.height + 32;
    state = nextPageHeight(state, report(height, { revision, viewportHeight: state.height }));
    expect(state.height).toBe(height);
    expect(state.held).toBe(false);
  }
});

test("independent growth at an unchanged viewport does not count as resize feedback", () => {
  let state = initialPageHeight();
  for (const height of [336, 352, 368, 384, 400, 416]) {
    state = nextPageHeight(state, report(height));
    expect(state.height).toBe(height);
    expect(state.growthSteps).toBe(0);
  }
});

test("an unchanged-height resize report separates later growth from resize feedback", () => {
  let state = nextPageHeight(initialPageHeight(), report(336));
  for (let i = 0; i < 10; i++) {
    state = nextPageHeight(state, report(state.height, { viewportHeight: state.height }));
    expect(state.viewportHeight).toBe(state.height);
    const height = state.height + 16;
    state = nextPageHeight(state, report(height, { viewportHeight: state.height }));
    expect(state.height).toBe(height);
    expect(state.held).toBe(false);
  }
});

test("new content releases a hold without a smaller report or document reload", () => {
  const state = heldPage();
  const grown = nextPageHeight(state, report(2_000, { revision: 1, viewportHeight: state.height }));
  expect(grown.height).toBe(2_000);
  expect(grown.held).toBe(false);
  expect(grown.growthSteps).toBe(0);
});

test("a content revision at unchanged height clears a hold before later growth", () => {
  const state = heldPage();
  const refreshed = nextPageHeight(state, report(state.height, { revision: 1, viewportHeight: state.height }));
  expect(refreshed.held).toBe(false);
  expect(nextPageHeight(refreshed, report(2_000, { revision: 1, viewportHeight: state.height })).height).toBe(2_000);
});

test("a width change releases a hold for a new layout", () => {
  const state = heldPage();
  const reflowed = nextPageHeight(state, report(2_000, { viewportWidth: 400, viewportHeight: state.height }));
  expect(reflowed.height).toBe(2_000);
  expect(reflowed.held).toBe(false);
});

test("growth with different deltas can still reach a legitimately long height", () => {
  let state = initialPageHeight();
  for (const height of [1_000, 2_100, 3_600, 9_000, 50_000]) {
    state = nextPageHeight(state, report(height, { viewportHeight: state.height }));
    expect(state.height).toBe(height);
    expect(state.held).toBe(false);
  }
});

test("a smaller report releases a hold and a new document gets a fresh guard", () => {
  const state = heldPage();
  const shrunk = nextPageHeight(state, report(200));
  expect(shrunk.held).toBe(false);
  expect(nextPageHeight(shrunk, report(2_000)).height).toBe(2_000);
  expect(nextPageHeight(initialPageHeight(state.height), report(2_000)).height).toBe(2_000);
});

test("nonlinear runaway growth reaches the emergency ceiling and can shrink", () => {
  let state = initialPageHeight();
  for (const height of [640, 1_280, 2_560, 5_120, 10_240, 20_480, 40_960, 81_920, 163_840]) {
    state = nextPageHeight(state, report(height, { viewportHeight: state.height }));
  }
  expect(state.height).toBe(RUNAWAY_HEIGHT_CEILING);
  expect(nextPageHeight(state, report(1_000_000)).height).toBe(RUNAWAY_HEIGHT_CEILING);
  expect(nextPageHeight(state, report(1_000)).height).toBe(1_000);
});

test("nonfinite heights leave the sizing state unchanged", () => {
  const state = initialPageHeight();
  for (const height of [NaN, Infinity, -Infinity]) expect(nextPageHeight(state, report(height))).toBe(state);
});

const ownFrame = {} as Window;
const otherFrame = {} as Window;
const message = (source: Window, values: Record<string, unknown>, type = HEIGHT_MESSAGE) => ({
  source, data: { type, ...values },
});

test("only the inline frame's own height reports are accepted", () => {
  const values = report(2_000);
  expect(readPageHeight(message(ownFrame, values), ownFrame)).toEqual(values);
  expect(readPageHeight(message(otherFrame, values), ownFrame)).toBeNull();
  expect(readPageHeight(message(ownFrame, values), null)).toBeNull();
  expect(readPageHeight(message(ownFrame, values, "another-message"), ownFrame)).toBeNull();
});

test("malformed height and feedback metadata are ignored", () => {
  const badFields = {
    height: [undefined, null, "123", "bad", NaN, Infinity],
    viewportHeight: [undefined, null, "320", 0, -1, Infinity],
    viewportWidth: [undefined, null, 0, NaN],
    revision: [undefined, null, "0", -1, 0.5, Infinity, Number.MAX_SAFE_INTEGER + 1],
  };
  for (const [key, values] of Object.entries(badFields)) {
    for (const value of values) {
      expect(readPageHeight(message(ownFrame, { ...report(500), [key]: value }), ownFrame)).toBeNull();
    }
  }
  expect(readPageHeight({ source: ownFrame, data: null }, ownFrame)).toBeNull();
});

test("only the inline document hides root and body overflow", () => {
  const html = '<!doctype html><html><head><title>Page</title></head><body>Content</body></html>';
  const inline = prepare(html, "https://example.test/outputs/", true, true);
  const expanded = prepare(html, "https://example.test/outputs/", true);
  expect(inline).toContain("html,body{overflow:hidden!important}");
  expect(expanded).not.toContain("overflow:hidden");
  for (const doc of [inline, expanded]) {
    expect(doc.startsWith("<!doctype html>")).toBe(true);
    expect(doc).toContain('base href="https://example.test/outputs/"');
    expect(doc).toContain('name="color-scheme" content="dark"');
    expect(doc).toContain(HEIGHT_MESSAGE);
  }
});
