import { describe, expect, test } from "bun:test";
import type { HarnessItem, ToolKind } from "@/lib/harness";
import { describeOculusCli } from "@/lib/harness";
import { bundleLabel } from "@/components/harness/timeline/rows";

let next = 1;
const tool = (kind: ToolKind, content = "x"): HarnessItem => ({
  id: next++,
  thread_id: 1,
  kind: "tool",
  ref_id: null,
  content,
  meta: JSON.stringify({ kind, ok: true }),
  created_at: "2026-11-15 00:00:00",
});
const thought = (ms: number): HarnessItem => ({
  id: next++,
  thread_id: 1,
  kind: "thinking",
  ref_id: null,
  content: "hm",
  meta: JSON.stringify({ ms }),
  created_at: "2026-11-15 00:00:00",
});
const reply = (): HarnessItem => ({
  id: next++,
  thread_id: 1,
  kind: "assistant",
  ref_id: null,
  content: "so far",
  meta: null,
  created_at: "2026-11-15 00:00:00",
});
const label = (items: HarnessItem[]) => bundleLabel(items).label;

describe("bundleLabel", () => {
  test("two categories join with and", () => {
    expect(label([tool("bash"), tool("edit"), tool("bash")])).toBe("Ran 2 commands and edited 1 file");
  });

  test("three categories keep the two that rank highest and count the rest", () => {
    // Reads come first but rank below commands and edits.
    expect(label([tool("read"), tool("read"), tool("bash"), tool("edit"), tool("write")])).toBe(
      "Ran 1 command, edited 2 files and performed 2 other actions",
    );
  });

  test("Performed 1 other action", () => {
    expect(label([tool("search"), tool("bash"), tool("read")])).toBe(
      "1 search, ran 1 command and performed 1 other action",
    );
  });

  test("shown clauses keep the order they were first seen", () => {
    expect(label([tool("edit"), tool("bash")])).toBe("Edited 1 file and ran 1 command");
  });

  test("an oculus write outranks reads", () => {
    const items = [tool("read"), tool("oculus_cli", "oculus search 'x'"), tool("oculus_cli", "oculus memory write notes")];
    expect(label(items)).toBe("Explored 1 file, saved a memory and performed 1 other action");
  });

  test("oculus reads rank with reads, and keep their own tallies", () => {
    const items = [tool("oculus_cli", "oculus search 'a'"), tool("oculus_cli", "oculus search 'b'"), tool("bash")];
    expect(label(items)).toBe("Searched the library 2 times and ran 1 command");
  });

  test("the rest ranks below reads", () => {
    expect(label([tool("other"), tool("plan"), tool("read"), tool("search")])).toBe(
      "Explored 1 file, 1 search and performed 2 other actions",
    );
  });

  test("reasoning in a mixed bundle is not counted", () => {
    expect(label([thought(1000), tool("bash"), thought(2000)])).toBe("Ran 1 command");
  });

  test("thought-only and reply-only bundles keep their labels", () => {
    expect(label([thought(1000), thought(2000)])).toMatch(/^Thought/);
    expect(label([reply(), reply()])).toBe("2 earlier replies");
    expect(label([reply()])).toBe("Earlier reply");
  });

  test("the glyph is the highest-ranked clause's", () => {
    expect(bundleLabel([tool("read"), tool("read"), tool("bash")]).icon).toEqual({ tool: "bash" });
    expect(bundleLabel([tool("read"), tool("oculus_cli", "oculus task add --title x")]).icon).toEqual({ cli: "task" });
  });
});

describe("CliAction.writes", () => {
  const writes = (line: string) => describeOculusCli(line)[0]?.writes ?? false;
  test("commands that change the library write", () => {
    expect(writes("oculus memory write x")).toBe(true);
    expect(writes("oculus task move 3 -c done")).toBe(true);
    expect(writes("oculus project create --title x")).toBe(true);
    expect(writes("oculus run")).toBe(true);
  });
  test("reads do not", () => {
    expect(writes("oculus search x")).toBe(false);
    expect(writes("oculus read courses/A/x.md")).toBe(false);
    expect(writes("oculus memory list")).toBe(false);
    expect(writes("oculus task list")).toBe(false);
  });
});
