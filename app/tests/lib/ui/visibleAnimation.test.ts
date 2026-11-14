import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { animationRuns, type AnimationGate } from "@/lib/ui/visibleAnimation";
import { runningSince, type HarnessRun } from "@/lib/harness";

const seen: AnimationGate = { intersecting: true, documentVisible: true, reducedMotion: false, tabActive: true };

describe("animationRuns", () => {
  test("runs only while on screen, visible, in the front tab and with motion allowed", () => {
    expect(animationRuns(seen)).toBe(true);
    expect(animationRuns({ ...seen, intersecting: false })).toBe(false);
    expect(animationRuns({ ...seen, documentVisible: false })).toBe(false);
    expect(animationRuns({ ...seen, reducedMotion: true })).toBe(false);
  });

  test("a hidden tab pane pauses even while its row still intersects", () => {
    expect(animationRuns({ ...seen, tabActive: false })).toBe(false);
  });
});

describe("runningSince", () => {
  const run = (id: number, status: HarnessRun["status"], started: number | null): HarnessRun => ({
    id, status, delivery: "unknown", answer_item_id: null, started_ms: started, ended_ms: null,
  });
  test("the newest running run's start", () => {
    expect(runningSince({ 1: run(1, "completed", 10), 2: run(2, "running", 20), 3: run(3, "running", 30) })).toBe(30);
  });
  test("null with no running run, or no runs", () => {
    expect(runningSince({ 1: run(1, "completed", 10) })).toBeNull();
    expect(runningSince(undefined)).toBeNull();
  });
});

describe("text-shimmer", () => {
  const css = readFileSync(new URL("../../../src/styles/utilities.css", import.meta.url), "utf8");
  const rule = css.slice(css.indexOf("@utility text-shimmer"), css.indexOf("@keyframes text-shimmer"));
  test("reads its play state from the gate", () => {
    expect(rule).toContain("animation-play-state: var(--shimmer-state, running)");
  });
  test("sweeps a neutral band, never the brand", () => {
    expect(rule).toContain("var(--color-foreground)");
    expect(rule).not.toMatch(/brand|primary/);
  });
});
