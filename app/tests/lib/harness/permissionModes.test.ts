import { describe, expect, test } from "bun:test";
import { modeChoices, threadMode } from "@/lib/harness";

describe("the mode control's lines per bridge", () => {
  test("an agent with a reviewer: the three modes with their own lines", () => {
    const { note, choices } = modeChoices({ provider: "codex", approvals: true, autoReview: true });
    expect(note).toBeNull();
    expect(choices).toEqual([
      { mode: "ask", label: "Ask", line: "You approve what the sandbox blocks" },
      { mode: "auto", label: "Auto", line: "The agent's reviewer decides" },
      { mode: "quiet", label: "Don't ask", line: "Blocked actions are declined" },
    ]);
  });

  test("Claude's Auto says its classifier decides", () => {
    const { note, choices } = modeChoices({ provider: "claude", approvals: true, autoReview: true });
    expect(note).toBeNull();
    expect(choices[1].line).toBe("Classifier decides");
    expect(choices[0].line).toBe("You approve what the sandbox blocks");
  });

  test("no reviewer: Auto says it asks as Ask", () => {
    const { note, choices } = modeChoices({ provider: "opencode", approvals: true, autoReview: false });
    expect(note).toBeNull();
    expect(choices[1].line).toBe("opencode has no reviewer, so it asks");
  });

  test("no approvals: the control says the agent cannot ask yet", () => {
    const { note } = modeChoices({ provider: "claude", approvals: false, autoReview: false });
    expect(note).toBe("Claude Code cannot ask yet: anything its sandbox blocks is declined, whichever you choose.");
  });

  test("a chat with no saved mode asks", () => {
    expect(threadMode(undefined)).toBe("ask");
    expect(threadMode("auto")).toBe("auto");
  });
});
