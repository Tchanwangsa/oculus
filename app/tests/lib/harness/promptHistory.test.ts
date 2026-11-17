import { describe, expect, test } from "bun:test";

import { pastedBlock, withAttachments } from "@/lib/harness/attachments";
import { promptHistoryEntries, recallablePrompt, stepPromptHistory, type PromptHistoryEntry } from "@/lib/harness/promptHistory";

describe("recallablePrompt", () => {
  test("drops pasted blocks and the picture line, keeps mentions and skills", () => {
    const typed = "/oculus-plan read `courses/MAST20004/Course/Pages/week-1.md`";
    const message = withAttachments(`${typed}\n\n${pastedBlock("a long paste")}`, ["./attachments/1.png"]);
    expect(recallablePrompt(message)).toBe(typed);
  });

  test("a message of only pictures or pastes has nothing to recall", () => {
    expect(recallablePrompt(withAttachments("", ["./attachments/1.png"]))).toBe("");
    expect(recallablePrompt(pastedBlock("only a paste"))).toBe("");
    expect(recallablePrompt("`./attachments/1.png` `./attachments/2.pdf`")).toBe("");
  });

  test("strips attachment chips from the sentence, closing each gap, and keeps mentions", () => {
    const sent = "compare `./attachments/1.png` with `./attachments/2.png` in `courses/MAST20004/Course/Pages/week-1.md`";
    expect(recallablePrompt(sent)).toBe("compare with in `courses/MAST20004/Course/Pages/week-1.md`");
    expect(recallablePrompt("`./attachments/3.pdf` summarise this\nthen `./attachments/4.md`")).toBe("summarise this\nthen");
  });
});

describe("promptHistoryEntries", () => {
  test("keeps the student's rows, skips blanks and collapses repeats", () => {
    const rows = [
      { id: 1, kind: "user", content: "first" },
      { id: 2, kind: "assistant", content: "reply" },
      { id: 3, kind: "user", content: "again" },
      { id: 4, kind: "continuation", content: "Sync finished — continuing" },
      { id: 5, kind: "user", content: "again" },
      { id: 6, kind: "user", content: withAttachments("", ["./attachments/1.png"]) },
      { id: 7, kind: "user", content: null },
    ];
    expect(promptHistoryEntries(rows)).toEqual([
      { id: 1, prompt: "first" },
      { id: 5, prompt: "again" },
    ]);
  });
});

describe("stepPromptHistory", () => {
  const entries: PromptHistoryEntry[] = [
    { id: 1, prompt: "one" },
    { id: 2, prompt: "two" },
    { id: 3, prompt: "three" },
  ];

  test("walks back from an empty box and forward to empty again", () => {
    let step = stepPromptHistory({ direction: "backward", entries, position: null, current: "" })!;
    expect(step.prompt).toBe("three");
    step = stepPromptHistory({ direction: "backward", entries, position: step.position, current: "three" })!;
    expect(step.prompt).toBe("two");
    step = stepPromptHistory({ direction: "forward", entries, position: step.position, current: "two" })!;
    expect(step.prompt).toBe("three");
    step = stepPromptHistory({ direction: "forward", entries, position: step.position, current: "three" })!;
    expect(step).toEqual({ position: null, prompt: "" });
  });

  test("stops at the oldest", () => {
    const step = stepPromptHistory({ direction: "backward", entries, position: { entryId: 1, recalled: "one" }, current: "one" });
    expect(step).toBeNull();
  });

  test("leaves the keys to the editor once there is other text", () => {
    expect(stepPromptHistory({ direction: "backward", entries, position: null, current: "typing" })).toBeNull();
    expect(stepPromptHistory({ direction: "forward", entries, position: null, current: "" })).toBeNull();
    // An edit to a recalled entry ends the walk.
    expect(stepPromptHistory({ direction: "backward", entries, position: { entryId: 3, recalled: "three" }, current: "three!" })).toBeNull();
  });

  test("an empty thread has nothing to recall", () => {
    expect(stepPromptHistory({ direction: "backward", entries: [], position: null, current: "" })).toBeNull();
  });
});
