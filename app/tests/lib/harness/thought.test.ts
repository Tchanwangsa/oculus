import { describe, expect, test } from "bun:test";
import { latestThought, thoughtLabel, thoughtMs, type HarnessItem } from "@/lib/harness";

describe("reasoning rows", () => {
  test("the label reads the stored duration", () => {
    expect(thoughtLabel(null)).toBe("Thought");
    expect(thoughtLabel(200)).toBe("Thought for 1s");
    expect(thoughtLabel(12_400)).toBe("Thought for 12s");
    expect(thoughtLabel(60_000)).toBe("Thought for 1m");
    expect(thoughtLabel(65_000)).toBe("Thought for 1m 5s");
    expect(thoughtMs({ meta: '{"ms":1200}' } as HarnessItem)).toBe(1200);
    expect(thoughtMs({ meta: null } as HarnessItem)).toBeNull();
    expect(thoughtMs({ meta: '{"ms":"x"}' } as HarnessItem)).toBeNull();
  });

  test("a live thought shows its latest readable sentence, as plain text", () => {
    expect(latestThought("**Checking the exam.** The lecturer says both are given.")).toBe("The lecturer says both are given.");
    // A sentence just begun would flicker; the finished one before it holds.
    expect(latestThought("The lecturer says both are given. With the")).toBe("The lecturer says both are given.");
    expect(latestThought("First paragraph ends here.\n\nNew")).toBe("First paragraph ends here.");
    expect(latestThought("- With the control in $|+\\rangle$ you need linearity to get the output"))
      .toBe("With the control in |+\\rangle you need linearity to get the output");
    expect(latestThought("")).toBe("");
  });
});
