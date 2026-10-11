import { describe, expect, spyOn, test } from "bun:test";

import { createDocumentSessions } from "@/lib/notes/documentSessions";
import { AUTO_SNAPSHOT_MS, type SnapshotReason } from "@/lib/notes/documentVersions";
import { NOTE, fakeDisk, listener, opened, tick, type Note, FakeView } from "./sessionHarness";

/** A fake disk whose session records the snapshots it asks for, on a clock
 *  the test moves. */
function snapshotting(initial: Record<string, string>) {
  const disk = fakeDisk(initial);
  const clock = { now: 1_000_000 };
  const snapshots: { id: number; text: string; reason: SnapshotReason }[] = [];
  const sessions = createDocumentSessions<Note>({
    ...disk.io,
    now: () => clock.now,
    snapshot: (file, text, reason) => void snapshots.push({ id: file.id, text, reason }),
  });
  return { disk, clock, snapshots, sessions };
}

describe("document session snapshots", () => {
  test("the first read asks for an open snapshot; a second editor does not", async () => {
    const { disk, snapshots, sessions } = snapshotting({ [NOTE.relative_path]: "hello" });
    const lease = sessions.open(NOTE, listener());
    const loading = lease.load();
    await tick();
    expect(snapshots).toEqual([]);
    disk.reads[0].done.resolve();
    await loading;
    expect(snapshots).toEqual([{ id: NOTE.id, text: "hello", reason: "open" }]);

    await sessions.open(NOTE, listener()).load();
    expect(snapshots.length).toBe(1);
  });

  test("a write asks for an interval snapshot once the interval has passed", async () => {
    const { disk, clock, snapshots, sessions } = snapshotting({ [NOTE.relative_path]: "" });
    const { lease, view } = await opened(sessions, disk);

    view.type(0, "a");
    clock.now += AUTO_SNAPSHOT_MS - 1;
    const first = lease.flush();
    disk.writes[0].done.resolve();
    await first;
    expect(snapshots.map((s) => s.reason)).toEqual(["open"]);

    view.type(1, "b");
    clock.now += 1;
    const second = lease.flush();
    disk.writes[1].done.resolve();
    await second;
    expect(snapshots.at(-1)).toEqual({ id: NOTE.id, text: "ab", reason: "interval" });

    // The interval restarts from that snapshot.
    view.type(2, "c");
    clock.now += AUTO_SNAPSHOT_MS - 1;
    const third = lease.flush();
    disk.writes[2].done.resolve();
    await third;
    expect(snapshots.map((s) => s.reason)).toEqual(["open", "interval"]);
  });

  test("the session ending asks for a close snapshot of the text on disk", async () => {
    const { disk, snapshots, sessions } = snapshotting({ [NOTE.relative_path]: "one" });
    const { lease, view, unbind } = await opened(sessions, disk);
    view.type(3, " two");
    unbind();
    lease.release();
    await tick();
    await tick();
    // Still writing: the session lives on, and no close yet.
    expect(snapshots.map((s) => s.reason)).toEqual(["open"]);

    disk.writes[0].done.resolve();
    await tick();
    await tick();
    expect(sessions.sessions.has(NOTE.id)).toBe(false);
    expect(snapshots.at(-1)).toEqual({ id: NOTE.id, text: "one two", reason: "close" });
  });

  test("no close snapshot while unsaved text keeps the session alive", async () => {
    const { disk, snapshots, sessions } = snapshotting({ [NOTE.relative_path]: "" });
    const { lease, view, unbind } = await opened(sessions, disk);
    view.type(0, "draft");
    unbind();
    lease.release();
    disk.writes[0].done.reject(new Error("offline"));
    await tick();
    await tick();
    expect(sessions.sessions.has(NOTE.id)).toBe(true);
    expect(snapshots.map((s) => s.reason)).toEqual(["open"]);
  });

  test("a failing snapshot never breaks loading or saving", async () => {
    const errors = spyOn(console, "error").mockImplementation(() => {});
    try {
      for (const snapshot of [
        () => {
          throw new Error("sync");
        },
        () => Promise.reject(new Error("async")),
      ]) {
        const disk = fakeDisk({ [NOTE.relative_path]: "x" });
        let clock = 0;
        const sessions = createDocumentSessions<Note>({
          ...disk.io,
          now: () => clock,
          snapshot,
        });
        const log: string[] = [];
        const lease = sessions.open(NOTE, listener(log));
        const loading = lease.load();
        await tick();
        disk.reads[0].done.resolve();
        expect(await loading).toBe("x");
        const view = new FakeView(lease.text!);
        view.lease = lease;
        lease.bind(view);

        view.type(1, "y");
        clock += AUTO_SNAPSHOT_MS;
        const saving = lease.flush();
        disk.writes[0].done.resolve();
        await saving;
        await tick();
        expect(disk.files[NOTE.relative_path]).toBe("xy");
        expect(log.at(-1)).toBe("saved");
        expect(lease.dirty).toBe(false);
      }
      // Open and interval, for each of the two.
      expect(errors).toHaveBeenCalledTimes(4);
    } finally {
      errors.mockRestore();
    }
  });
});
