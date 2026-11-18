import { describe, expect, test } from "bun:test";
import { deriveActivity } from "@/hooks/data/overview/activity";
import { parseEdThread } from "@/lib/ed/threads";
import { classes, date, file, first, iso, lecture, second, sqlite, strip } from "./fixtures";

const activity = (files: ReturnType<typeof file>[], key = first, since: number | null = null) =>
  deriveActivity(1, strip, key, files, [], classes, [], since);

describe("overview activity", () => {
  test("NULL source creation falls back to SQLite UTC and dots compare instants", () => {
    const [item] = activity([file()], first, date().getTime());
    expect(item.date.getTime()).toBe(date().getTime());
    expect(item.isNew).toBe(false);
    expect(activity([file()], first, date().getTime() - 1)[0].isNew).toBe(true);
    expect(activity([file()])[0].isNew).toBe(true);
    expect(activity([file({ first_seen_at: null })])).toEqual([]);
  });

  test("source creation wins over discovery and changes must exceed one hour", () => {
    const old = file({ source_created_at: iso(6), first_seen_at: sqlite(20) });
    expect(activity([old])[0].badge).toBe("New");
    expect(activity([old], second)).toEqual([]);
    expect(activity([file({ source_created_at: iso(6, 10), source_updated_at: iso(6, 11) })])[0].badge).toBe("New");
    expect(activity([file({ source_created_at: iso(6, 10), source_updated_at: iso(6, 12) })])[0].badge).toBe("Updated");
    const updated = file({ source_created_at: iso(6), source_updated_at: iso(20) });
    expect(activity([updated])[0].badge).toBe("New");
    expect(activity([updated], second)[0].badge).toBe("Updated");
    expect(activity([file({ source_created_at: "bad", first_seen_at: sqlite() })])).toEqual([]);
  });

  test("SQLite fallback and source ISO dates agree across a local Monday boundary", () => {
    const created = date(12, 0);
    const utc = created.toISOString();
    const row = file({ first_seen_at: utc.replace("T", " ").replace(".000Z", "") });
    expect(activity([row], first)).toEqual([]);
    const key = strip.weeks[1].key;
    expect(activity([row], key)[0].date.getTime()).toBe(created.getTime());
    expect(activity([file({ source_created_at: utc })], key)[0].date.getTime()).toBe(created.getTime());
  });

  test("announcements use their source creation, personal files stay in Yours", () => {
    const files = [file({ category: "announcement", source_created_at: iso(6), source_updated_at: iso(20) }),
      ...["document", "whiteboard", "notebook", "upload", "module"].map((category, id) => file({ id: id + 2, category })),
      file({ id: 10, subject_id: 2 })];
    expect(activity(files).map((item) => item.kind)).toEqual(["announcement"]);
    expect(activity(files, second)).toEqual([]);
  });

  test("Ed replies use source update without the one-hour threshold and parsed counts", () => {
    const row = file({ category: "ed", source_created_at: iso(6, 10), source_updated_at: iso(6, 10).replace(":00.000", ":30.000") });
    const parsed = parseEdThread("# Help\n\n---\n\nBody\n\n## Replies\n\n**Tutor** (answer) · 2026-10-06 10:00\nYes\n\n> **Student** · 2026-10-06 10:01\n> Thanks", row);
    const [item] = deriveActivity(1, strip, first, [row], [], classes, [parsed], null);
    expect(parsed.replyCount).toBe(2);
    expect(item.badge).toBe("2 replies");
    expect(item.title).toBe("Help");
    expect(deriveActivity(1, strip, first, [row], [], classes, [], null)[0].badge).toBe("Replies");
    expect(activity([file({ category: "ed", source_updated_at: null })])[0].badge).toBe("New thread");
  });

  test("recordings keep timetable status and activity sorts newest first", () => {
    const recordings = [lecture("2026-10-05T10:05:00"), lecture("2026-10-06T10:00:00"), lecture("bad")];
    const items = deriveActivity(1, strip, first, [file({ first_seen_at: sqlite(7) })], recordings, classes, [], null);
    expect(items.map((item) => item.recordingStatus)).toEqual([null, "untimetabled", "timetabled"]);
    expect(items[1].badge).toBe("New recording");
  });

  test("edge activity folds into week 1 and +3; empty strips never match", () => {
    expect(activity([file({ source_created_at: iso(1) })])[0].date.getTime()).toBe(date(1).getTime());
    const key = strip.weeks.at(-1)!.key;
    expect(activity([file({ source_created_at: "2027-01-01T00:00:00.000Z" })], key)).toHaveLength(1);
    expect(deriveActivity(1, { ...strip, weeks: [], current: -1 }, null, [file()], [], [], [], null)).toEqual([]);
  });
});
