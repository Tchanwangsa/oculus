import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { RateWindow, ThreadUsage } from "@/lib/harness";
import { pageKey, rowStep, slideFrom } from "@/components/harness/composer/settings/pages";

// The menu's import graph reaches modules that listen on `window` at load.
const globals = globalThis as { window?: unknown; document?: unknown };
const before = { window: globals.window, document: globals.document };
const inert = { addEventListener: () => {}, removeEventListener: () => {} };
globals.window ??= inert;
globals.document ??= { ...inert, documentElement: { style: {} } };
const { SettingsList } = await import("@/components/harness/composer/settings/CompactSettings");
const { UsageMeter, usageSummary } = await import("@/components/harness/composer/UsageMeter");
Object.assign(globals, before);

const noop = () => {};
const windows: RateWindow[] = [
  { label: "5-hour", used_percent: 27, resets_at: null },
  { label: "Weekly", used_percent: 62, resets_at: null },
];
const usage = { contextTokens: 50_000, contextWindow: 200_000 } as ThreadUsage;

const list = (rateLimits: RateWindow[], u: ThreadUsage | null = usage) =>
  renderToStaticMarkup(
    <SettingsList
      model={{ label: "Opus 5.5", level: "Medium", title: "Claude Code · opus · Medium reasoning" }}
      provider="claude"
      mode="ask"
      usage={usageSummary(u, rateLimits)}
      onOpen={noop}
    />,
  );

describe("the settings list", () => {
  test("a row each for model, permission mode and usage, each opening its page", () => {
    const html = list(windows);
    expect(html).toContain('data-settings-row="model"');
    expect(html).toContain('data-settings-row="permission"');
    expect(html).toContain('data-settings-row="usage"');
    expect(html).toContain(">Opus 5.5<");
    expect(html).toContain(">Medium<");
    expect(html).toContain(">Permission mode<");
    expect(html).toContain(">Ask<");
  });

  test("usage names only the window nearest its limit", () => {
    const html = list(windows);
    expect(html).toContain(">Weekly 62%<");
    expect(html).not.toContain("5-hour 27%");
    expect(html).toContain('title="Weekly limit 62% used · 25% of context"');
  });

  test("the context window is always at the foot, with or without plan windows", () => {
    expect(list(windows)).toContain(">50k / 200k<");
    expect(list([])).toContain(">50k / 200k<");
    expect(list([])).toContain('title="Context: 25% used"');
  });

  test("without plan windows there is no usage row; with nothing at all no context either", () => {
    expect(list([])).not.toContain('data-settings-row="usage"');
    expect(list([], null)).not.toContain(">Context<");
  });
});

describe("pages", () => {
  test("Escape and Left go back from a page; on the list they are the popover's", () => {
    expect(pageKey("model", "Escape")).toBe("back");
    expect(pageKey("usage", "ArrowLeft")).toBe("back");
    expect(pageKey("root", "Escape")).toBeNull();
    expect(pageKey("root", "ArrowLeft")).toBeNull();
  });

  test("Left stays with a text field or a control that took it", () => {
    expect(pageKey("model", "ArrowLeft", { editing: true })).toBeNull();
    expect(pageKey("model", "ArrowLeft", { handled: true })).toBeNull();
    // Escape still goes back from the search box.
    expect(pageKey("model", "Escape", { editing: true })).toBe("back");
  });

  test("a row's page slides in from the right, the list from the left", () => {
    expect(slideFrom("permission")).toBe("right");
    expect(slideFrom("root")).toBe("left");
  });

  test("Up and Down walk the rows and wrap", () => {
    expect(rowStep("ArrowDown", 2, 3)).toBe(0);
    expect(rowStep("ArrowUp", 0, 3)).toBe(2);
    expect(rowStep("End", 0, 3)).toBe(2);
    expect(rowStep("Enter", 0, 3)).toBeNull();
    expect(rowStep("ArrowDown", 0, 0)).toBeNull();
  });
});

describe("the full composer's ring", () => {
  const ring = (rateLimits: RateWindow[]) => renderToStaticMarkup(<UsageMeter usage={usage} rateLimits={rateLimits} />);

  test("is the plan window closest to running out, says which, and names the context too", () => {
    const html = ring(windows);
    expect(html).toContain('aria-label="Usage — Weekly limit 62% used · 25% of context"');
    expect(usageSummary(usage, windows).percent).toBe(62);
  });

  test("falls back to the context without plan windows", () => {
    expect(ring([])).toContain('aria-label="Usage — 25% of context"');
    expect(usageSummary(usage, []).percent).toBe(25);
  });
});
