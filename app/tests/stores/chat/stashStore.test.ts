import { describe, expect, test } from "bun:test";

import { isStashKey } from "@/components/harness/composer/useComposerStash";
import { attachmentRefs } from "@/lib/harness/attachments";
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

describe("a stashed draft with attachments", () => {
  test("keeps its chips' paths in its words and its shelf as paths, written already", () => {
    const store = useStashStore.getState();
    const text = "compare `./attachments/1700-1.png` with `./attachments/1700-2.pdf`";
    store.stash({ text, pastes: [], pictures: ["./attachments/1700-1.png", "./attachments/1700-3.png"] });
    const entry = useStashStore.getState().entries[0];
    expect(entry.text).toBe(text);
    expect(entry.pictures).toEqual(["./attachments/1700-1.png", "./attachments/1700-3.png"]);
    expect(attachmentRefs(entry.text).map((r) => r.raw)).toEqual(["./attachments/1700-1.png", "./attachments/1700-2.pdf"]);
    store.remove(entry.id);
  });
});

describe("stashSnippet", () => {
  test("names the words, else what the entry holds", () => {
    expect(stashSnippet({ text: "  a\n b ", pastes: [], pictures: [] })).toBe("a b");
    expect(stashSnippet({ text: "x".repeat(100), pastes: [], pictures: [] })).toBe(`${"x".repeat(90)}…`);
    expect(stashSnippet({ text: "", pastes: ["p"], pictures: ["./attachments/a.png", "./attachments/b.png"] })).toBe("(2 pictures, 1 pasted text)");
    expect(stashSnippet({ text: "", pastes: [], pictures: [] })).toBe("(empty)");
    const text = "compare `./attachments/a.png` with `./attachments/b.pdf` in `courses/X/Course/Files/w.pdf`";
    expect(stashSnippet({ text, pastes: [], pictures: [] }, 90, (p) => (p.endsWith("a.png") ? "Figure 1.png" : undefined))).toBe(
      "compare Figure 1.png with PDF in `courses/X/Course/Files/w.pdf`",
    );
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
