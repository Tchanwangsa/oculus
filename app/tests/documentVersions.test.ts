import { afterAll, beforeAll, describe, expect, test } from "bun:test";

import { lastNoteWrite, quickHash, saveDocument } from "../src/lib/documents";
import type { DbFile } from "../src/lib/db";
import {
  changedOutside,
  copySuffix,
  textHash,
  versionsToPrune,
  versionTitle,
  type DocumentVersion,
} from "../src/lib/documentVersions";

// Titles are local time; pin the zone so 03:02 UTC reads as 2:02 pm (AEDT).
// Test files share a process, so put it back after.
const zone = process.env.TZ;
beforeAll(() => {
  process.env.TZ = "Australia/Melbourne";
});
afterAll(() => {
  if (zone === undefined) delete process.env.TZ;
  else process.env.TZ = zone;
});

const HOUR = 3_600_000;
const DAY = 24 * HOUR;
const NOW = new Date(Date.UTC(2026, 9, 5, 12, 0, 0));

/** SQLite's `datetime('now')` shape for an instant. */
const sqlite = (ms: number) => new Date(ms).toISOString().slice(0, 19).replace("T", " ");

let nextId = 1;
function version(ago: number, over: Partial<DocumentVersion> = {}): DocumentVersion {
  return {
    id: nextId++,
    file_id: 1,
    number: null,
    label: null,
    kind: "auto",
    hash: "h",
    created_at: sqlite(NOW.getTime() - ago),
    length: 0,
    ...over,
  };
}

describe("versionsToPrune", () => {
  test("keeps every snapshot younger than 24 h", () => {
    const vs = [version(0), version(HOUR), version(23 * HOUR), version(DAY - 1000)];
    expect(versionsToPrune(vs, NOW)).toEqual([]);
  });

  test("past 24 h keeps the newest per UTC day", () => {
    // NOW is 12:00 UTC on 5 Oct, so 26 h and 30 h ago are both 4 Oct.
    const early = version(30 * HOUR);
    const late = version(26 * HOUR);
    const otherDay = version(40 * HOUR);
    expect(versionsToPrune([late, early, otherDay], NOW)).toEqual([early.id]);
    expect(versionsToPrune([early, late, otherDay], NOW)).toEqual([early.id]);
  });

  test("days are UTC days, not local ones", () => {
    // 4 Oct 23:30 UTC and 5 Oct 00:30 UTC share a Melbourne day but not a UTC one.
    const now = new Date(Date.UTC(2026, 9, 6, 12, 0, 0));
    const before = version(now.getTime() - Date.UTC(2026, 9, 4, 23, 30));
    const after = version(now.getTime() - Date.UTC(2026, 9, 5, 0, 30));
    expect(versionsToPrune([before, after], now)).toEqual([]);
  });

  test("a tie within the second keeps the higher id", () => {
    const a = version(2 * DAY);
    const b = version(2 * DAY);
    expect(versionsToPrune([a, b], NOW)).toEqual([a.id]);
    expect(versionsToPrune([b, a], NOW)).toEqual([a.id]);
  });

  test("drops snapshots older than 30 days, keeps one exactly 30 days old", () => {
    const edge = version(30 * DAY);
    const old = version(30 * DAY + 1000);
    const ancient = version(400 * DAY);
    expect(versionsToPrune([edge, old, ancient], NOW)).toEqual([old.id, ancient.id]);
  });

  test("every snapshot kind is thinned alike", () => {
    const external = version(26 * HOUR, { kind: "external" });
    const restore = version(30 * HOUR, { kind: "restore" });
    expect(versionsToPrune([external, restore], NOW)).toEqual([restore.id]);
  });

  test("checkpoints always stay, and do not count as a day's newest", () => {
    const checkpoints = [
      version(26 * HOUR, { kind: "checkpoint", number: 2 }),
      version(30 * HOUR, { kind: "checkpoint", number: 1 }),
      version(400 * DAY, { kind: "checkpoint", number: 3 }),
    ];
    const snap = version(28 * HOUR);
    expect(versionsToPrune([...checkpoints, snap], NOW)).toEqual([]);
  });
});

describe("versionTitle", () => {
  const at = sqlite(Date.UTC(2026, 9, 5, 3, 2));

  test("a checkpoint is its number and label", () => {
    const v = version(0, { kind: "checkpoint", number: 3, label: "Before the exam", created_at: at });
    expect(versionTitle(v)).toBe("v3 · Before the exam");
    expect(versionTitle({ ...v, label: null })).toBe("v3");
  });

  test("a snapshot is its local time", () => {
    expect(versionTitle(version(0, { created_at: at }))).toBe("5 Oct, 2:02 pm");
    expect(versionTitle(version(0, { kind: "external", label: "x", created_at: at }))).toBe(
      "5 Oct, 2:02 pm",
    );
  });

  test("just past midnight is 12 am on the new day", () => {
    // 13:05 UTC on 4 Oct is 00:05 on 5 Oct in Melbourne.
    expect(versionTitle(version(0, { created_at: sqlite(Date.UTC(2026, 9, 4, 13, 5)) }))).toBe(
      "5 Oct, 12:05 am",
    );
  });
});

describe("copySuffix", () => {
  test("is filename-safe", () => {
    const at = sqlite(Date.UTC(2026, 9, 5, 3, 2));
    expect(copySuffix(version(0, { kind: "checkpoint", number: 3, label: "a/b: c" }))).toBe("v3");
    expect(copySuffix(version(0, { created_at: at }))).toBe("5 Oct 2.02 pm");
  });
});

describe("textHash", () => {
  test("is the sha-256 hex of the text", async () => {
    expect(await textHash("")).toBe(
      "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );
    expect(await textHash("abc")).toBe(
      "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    );
    expect(await textHash("abc ")).not.toBe(await textHash("abc"));
  });
});

describe("telling the app's writes from others'", () => {
  /** A fresh `localStorage` for the test's length (test files share a
   *  process, and another may have installed one); `null` makes it throw. */
  function withStorage(run: () => Promise<void>, storage: "fresh" | null = "fresh") {
    return async () => {
      const items = new Map<string, string>();
      const g = globalThis as { localStorage?: unknown };
      const before = g.localStorage;
      const refuse = () => {
        throw new Error("storage unavailable");
      };
      g.localStorage = {
        getItem: storage ? (k: string) => items.get(k) ?? null : refuse,
        setItem: storage ? (k: string, v: string) => void items.set(k, v) : refuse,
      };
      try {
        await run();
      } finally {
        g.localStorage = before;
      }
    };
  }

  const note = { id: 7, relative_path: "courses/X/documents/a.md" } as DbFile;
  /** `saveDocument` with no Tauri to answer: the write is issued, never lands. */
  const issue = (text: string) => saveDocument(note, text).catch(() => {});

  test(
    "a write is recorded as it is issued, before any reply",
    withStorage(async () => {
      expect(lastNoteWrite(note.id)).toBeNull();
      await issue("draft");
      expect(lastNoteWrite(note.id)).toBe(quickHash("draft"));
    }),
  );

  test(
    "the app's own save, unsnapshotted before a quit, opens as auto",
    withStorage(async () => {
      await issue("v1");
      await issue("v2");
      expect(changedOutside(lastNoteWrite(note.id), "v2")).toBe(false);
    }),
  );

  test(
    "another writer's text opens as external",
    withStorage(async () => {
      await issue("v2");
      expect(changedOutside(lastNoteWrite(note.id), "v2, edited by an agent")).toBe(true);
    }),
  );

  test(
    "with no record of a write, nothing is external",
    withStorage(async () => {
      expect(changedOutside(lastNoteWrite(note.id), "anything")).toBe(false);
    }),
  );

  test(
    "without storage, saving still issues the write and nothing is external",
    withStorage(async () => {
      expect(lastNoteWrite(note.id)).toBeNull();
      await issue("x");
      expect(changedOutside(lastNoteWrite(note.id), "y")).toBe(false);
    }, null),
  );

  test("quickHash is FNV-1a and tells near texts apart", () => {
    expect(quickHash("")).toBe("811c9dc5");
    expect(quickHash("a")).toBe("e40c292c");
    expect(quickHash("abc ")).not.toBe(quickHash("abc"));
  });
});
