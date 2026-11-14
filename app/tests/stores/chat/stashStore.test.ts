import { describe, expect, test } from "bun:test";

import { isStashKey } from "@/components/harness/composer/useComposerStash";
import { keptAttachments, writeAttachment } from "@/lib/harness/attachments";
import { MAX_STASH, pushStash, stashSnippet, useStashStore, type StashEntry } from "@/stores/chat/stashStore";

const entry = (n: number): StashEntry => ({ id: `e${n}`, createdAt: n, text: `prompt ${n}`, pastes: [], pictures: [] });

describe("pushStash", () => {
  test("keeps the newest first and drops the oldest past twenty", () => {
    let entries: StashEntry[] = [];
    for (let n = 1; n <= MAX_STASH; n++) {
      const pushed = pushStash(entries, entry(n));
      expect(pushed.evicted).toBe(false);
      entries = pushed.entries;
    }
    expect(entries).toHaveLength(20);
    const full = pushStash(entries, entry(21));
    expect(full.evicted).toBe(true);
    expect(full.entries).toHaveLength(20);
    expect(full.entries[0].id).toBe("e21");
    expect(full.entries.map((e) => e.id)).not.toContain("e1");
  });
});

describe("useStashStore", () => {
  test("a 21st stash sets the notice the menu shows until it closes", () => {
    const store = useStashStore.getState();
    for (let n = 0; n < MAX_STASH; n++) store.stash({ text: `p${n}`, pastes: [], pictures: [] });
    expect(useStashStore.getState().evicted).toBe(false);
    store.stash({ text: "one more", pastes: [], pictures: [] });
    expect(useStashStore.getState().entries).toHaveLength(MAX_STASH);
    expect(useStashStore.getState().entries[0].text).toBe("one more");
    expect(useStashStore.getState().evicted).toBe(true);
    useStashStore.getState().clearEvicted();
    expect(useStashStore.getState().evicted).toBe(false);
    const first = useStashStore.getState().entries[0];
    useStashStore.getState().remove(first.id);
    expect(useStashStore.getState().entries.map((e) => e.id)).not.toContain(first.id);
  });
});

describe("restoring a stashed picture", () => {
  test("comes back kept, and is sent again by the same path", async () => {
    const globals = globalThis as { window?: unknown };
    const before = globals.window;
    globals.window = { __TAURI_INTERNALS__: { convertFileSrc: (p: string) => `asset://${p}` } };
    try {
      const kept = keptAttachments(["./attachments/1700-1.png", "courses/X/not-a-picture.png"], "/data");
      expect(kept).toHaveLength(1);
      expect(kept[0].source).toEqual({ kind: "kept", path: "./attachments/1700-1.png" });
      expect(kept[0].preview).toBe("asset:///data/agents/attachments/1700-1.png");
      // A kept picture is never written again: stashing or sending it reuses its path.
      expect(await writeAttachment(kept[0])).toBe("./attachments/1700-1.png");
    } finally {
      globals.window = before;
    }
  });
});

describe("stashSnippet", () => {
  test("names the words, else what the entry holds", () => {
    expect(stashSnippet({ text: "  a\n b ", pastes: [], pictures: [] })).toBe("a b");
    expect(stashSnippet({ text: "x".repeat(100), pastes: [], pictures: [] })).toBe(`${"x".repeat(90)}…`);
    expect(stashSnippet({ text: "", pastes: ["p"], pictures: ["./attachments/a.png", "./attachments/b.png"] })).toBe("(2 pictures, 1 pasted text)");
    expect(stashSnippet({ text: "", pastes: [], pictures: [] })).toBe("(empty)");
  });
});

test("⌘S and Ctrl+S are the stash key; with Shift or Alt they are not", () => {
  const key = (over: Partial<Parameters<typeof isStashKey>[0]>) =>
    isStashKey({ key: "s", metaKey: false, ctrlKey: false, shiftKey: false, altKey: false, ...over });
  expect(key({ metaKey: true })).toBe(true);
  expect(key({ ctrlKey: true, key: "S" })).toBe(true);
  expect(key({ metaKey: true, shiftKey: true })).toBe(false);
  expect(key({ metaKey: true, altKey: true })).toBe(false);
  expect(key({})).toBe(false);
});
