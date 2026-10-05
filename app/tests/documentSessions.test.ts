import { describe, expect, test } from "bun:test";
import { history, undo } from "@codemirror/commands";
import { EditorState, type Transaction, type TransactionSpec } from "@codemirror/state";

import {
  createDocumentSessions,
  type DocumentLease,
  type NoteRef,
  type SessionListener,
} from "../src/lib/documentSessions";

type Note = NoteRef & { filename: string };

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

const tick = (ms = 0) => new Promise((resolve) => setTimeout(resolve, ms));

/** A disk whose reads and writes resolve only when the test says so. */
function fakeDisk(initial: Record<string, string>) {
  const files = { ...initial };
  const reads: { path: string; done: ReturnType<typeof deferred<void>> }[] = [];
  const writes: { path: string; text: string; done: ReturnType<typeof deferred<void>> }[] = [];
  return {
    files,
    reads,
    writes,
    io: {
      delay: 20,
      read: async (path: string) => {
        const done = deferred<void>();
        reads.push({ path, done });
        await done.promise;
        return files[path] ?? "";
      },
      write: async (file: Note, text: string) => {
        const done = deferred<void>();
        writes.push({ path: file.relative_path, text, done });
        await done.promise;
        files[file.relative_path] = text;
      },
    },
  };
}

/** An `EditorView` stand-in: a state, a dispatch, and the update listener
 *  `DocumentEditor` installs, feeding the lease. */
class FakeView {
  state: EditorState;
  lease: DocumentLease<Note> | null = null;
  constructor(doc: string) {
    this.state = EditorState.create({ doc, extensions: [history()] });
  }
  dispatch(spec: TransactionSpec | Transaction) {
    const tr = "startState" in spec ? (spec as Transaction) : this.state.update(spec);
    this.state = tr.state;
    if (tr.docChanged) this.lease?.changed(this, [tr]);
  }
  type(at: number, text: string) {
    this.dispatch({ changes: { from: at, insert: text } });
  }
  get doc() {
    return this.state.doc.toString();
  }
}

/** A minimal target for CodeMirror's `undo`, which reads `state` and dispatches. */
function withState(state: EditorState) {
  const target = { state, dispatch: (tr: Transaction) => void (target.state = tr.state) };
  return target;
}

function listener(log: string[] = []): SessionListener & { log: string[] } {
  return {
    log,
    status: (s) => log.push(s.state),
    saved: (text) => log.push(`saved:${text}`),
  };
}

const NOTE: Note = { id: 1, relative_path: "courses/X/documents/a.md", filename: "a.md" };

async function opened(sessions: ReturnType<typeof createDocumentSessions<Note>>, disk: ReturnType<typeof fakeDisk>) {
  const lease = sessions.open(NOTE, listener());
  const loading = lease.load();
  await tick();
  disk.reads.at(-1)?.done.resolve();
  await loading;
  const view = new FakeView(lease.text!);
  view.lease = lease;
  const unbind = lease.bind(view);
  return { lease, view, unbind };
}

describe("document sessions", () => {
  test("a second editor takes the session's unsaved text without a read", async () => {
    const disk = fakeDisk({ [NOTE.relative_path]: "hello" });
    const sessions = createDocumentSessions<Note>(disk.io);
    const { view } = await opened(sessions, disk);
    view.type(5, " world");

    const second = sessions.open(NOTE, listener());
    expect(second.text).toBe("hello world");
    expect(await second.load()).toBe("hello world");
    expect(disk.reads.length).toBe(1);
  });

  test("writes are serialised and loop until disk holds the latest text", async () => {
    const disk = fakeDisk({ [NOTE.relative_path]: "" });
    const sessions = createDocumentSessions<Note>(disk.io);
    const { lease, view } = await opened(sessions, disk);

    view.type(0, "a");
    const first = lease.flush();
    view.type(1, "b");
    const again = lease.flush();
    expect(again).toBe(first);
    expect(disk.writes.map((w) => w.text)).toEqual(["a"]);

    disk.writes[0].done.resolve();
    await tick();
    // The second write starts only after the first lands.
    expect(disk.writes.map((w) => w.text)).toEqual(["a", "ab"]);
    disk.writes[1].done.resolve();
    await first;
    expect(disk.files[NOTE.relative_path]).toBe("ab");
    expect(lease.dirty).toBe(false);
  });

  test("an edit is saved after the debounce, once", async () => {
    const disk = fakeDisk({ [NOTE.relative_path]: "" });
    const sessions = createDocumentSessions<Note>(disk.io);
    const { view } = await opened(sessions, disk);
    view.type(0, "x");
    view.type(1, "y");
    expect(disk.writes.length).toBe(0);
    await tick(40);
    expect(disk.writes.map((w) => w.text)).toEqual(["xy"]);
  });

  test("a failed save surfaces and waits for the next edit", async () => {
    const disk = fakeDisk({ [NOTE.relative_path]: "" });
    const sessions = createDocumentSessions<Note>(disk.io);
    const log: string[] = [];
    const lease = sessions.open(NOTE, listener(log));
    const loading = lease.load();
    await tick();
    disk.reads[0].done.resolve();
    await loading;
    const view = new FakeView(lease.text!);
    view.lease = lease;
    lease.bind(view);

    view.type(0, "x");
    const saving = lease.flush();
    disk.writes[0].done.reject(new Error("disk full"));
    await saving;
    expect(log.at(-1)).toBe("error");
    expect(lease.dirty).toBe(true);
    await tick(40);
    expect(disk.writes.length).toBe(1);

    view.type(1, "y");
    await tick(40);
    expect(disk.writes.at(-1)?.text).toBe("xy");
  });

  test("detach and re-attach in one tick keeps the session; idle and unheld drops it", async () => {
    const disk = fakeDisk({ [NOTE.relative_path]: "kept" });
    const sessions = createDocumentSessions<Note>(disk.io);
    const { lease, view, unbind } = await opened(sessions, disk);
    view.type(4, "!");

    // Fast Refresh: cleanups, then the same effects again, synchronously.
    unbind();
    lease.release();
    const again = sessions.open(NOTE, listener());
    expect(again.text).toBe("kept!");
    await tick();
    expect(sessions.sessions.has(NOTE.id)).toBe(true);

    // The release flushed: the write is in flight, so nothing reads.
    expect(disk.writes.map((w) => w.text)).toEqual(["kept!"]);
    disk.writes[0].done.resolve();
    again.release();
    await tick();
    await tick();
    expect(sessions.sessions.has(NOTE.id)).toBe(false);
    expect(disk.reads.length).toBe(1);
  });

  test("a session with unsaved text outlives its editors", async () => {
    const disk = fakeDisk({ [NOTE.relative_path]: "" });
    const sessions = createDocumentSessions<Note>(disk.io);
    const { lease, view, unbind } = await opened(sessions, disk);
    view.type(0, "draft");
    unbind();
    lease.release();
    disk.writes[0].done.reject(new Error("offline"));
    await tick();
    await tick();
    expect(sessions.sessions.has(NOTE.id)).toBe(true);
    expect(sessions.open(NOTE, listener()).text).toBe("draft");
  });

  test("views of one note stay identical with their own undo", async () => {
    const disk = fakeDisk({ [NOTE.relative_path]: "one" });
    const sessions = createDocumentSessions<Note>(disk.io);
    const { view: a } = await opened(sessions, disk);
    const lease = sessions.open(NOTE, listener());
    const b = new FakeView(lease.text!);
    b.lease = lease;
    lease.bind(b);

    a.type(3, " two");
    expect(b.doc).toBe("one two");
    b.type(0, "zero ");
    expect(a.doc).toBe("zero one two");

    // B's undo takes back only B's edit, and A follows.
    undo({ state: b.state, dispatch: (tr) => b.dispatch(tr) });
    expect(b.doc).toBe("one two");
    expect(a.doc).toBe("one two");

    await tick(40);
    expect(disk.writes.length).toBe(1);
    expect(disk.writes[0].text).toBe("one two");
  });

  test("a remounted editor keeps the undo history and caret the last one left", async () => {
    const disk = fakeDisk({ [NOTE.relative_path]: "hello" });
    const sessions = createDocumentSessions<Note>(disk.io);
    const { lease, view, unbind } = await opened(sessions, disk);
    view.type(5, " world");
    view.dispatch({ selection: { anchor: 3 } });
    unbind();

    // Fast Refresh: the old view goes and a new one attaches in the same tick.
    const again = lease.restore({ extensions: [history()] });
    expect(again.doc.toString()).toBe("hello world");
    expect(again.selection.main.head).toBe(3);
    const next = new FakeView("");
    next.state = again;
    next.lease = lease;
    lease.bind(next);
    expect(undo({ state: next.state, dispatch: (tr) => next.dispatch(tr) })).toBe(true);
    expect(next.doc).toBe("hello");
  });

  test("a remounted editor starts clean when the text moved on without it", async () => {
    const disk = fakeDisk({ [NOTE.relative_path]: "hello" });
    const sessions = createDocumentSessions<Note>(disk.io);
    const { lease, view, unbind } = await opened(sessions, disk);
    const split = new FakeView(lease.text!);
    split.lease = lease;
    lease.bind(split);
    view.type(5, " world");
    unbind();
    // The other pane keeps typing after the first one left its stash.
    split.type(0, ">");

    const fresh = lease.restore({ extensions: [history()] });
    expect(fresh.doc.toString()).toBe(">hello world");
    expect(undo(withState(fresh))).toBe(false);
  });

  test("a view bound behind the session catches up", async () => {
    const disk = fakeDisk({ [NOTE.relative_path]: "old" });
    const sessions = createDocumentSessions<Note>(disk.io);
    const { view: a } = await opened(sessions, disk);
    const lease = sessions.open(NOTE, listener());
    const stale = new FakeView("old");
    a.type(3, "er");
    stale.lease = lease;
    lease.bind(stale);
    expect(stale.doc).toBe("older");
  });

  test("a rename holds writes and later saves go to the new path", async () => {
    const disk = fakeDisk({ [NOTE.relative_path]: "" });
    const sessions = createDocumentSessions<Note>(disk.io);
    const { lease, view } = await opened(sessions, disk);
    const moved = deferred<void>();
    const renaming = lease.rename(async (file) => {
      await moved.promise;
      return { ...file, relative_path: "courses/X/documents/b.md", filename: "b.md" };
    });
    await tick();
    view.type(0, "during");
    await tick(40);
    expect(disk.writes.length).toBe(0);
    moved.resolve();
    expect(await renaming).toBe(true);
    await tick();
    expect(disk.writes.map((w) => w.path)).toEqual(["courses/X/documents/b.md"]);
  });

  test("a rename refuses to move a note whose save failed", async () => {
    const disk = fakeDisk({ [NOTE.relative_path]: "" });
    const sessions = createDocumentSessions<Note>(disk.io);
    const { lease, view } = await opened(sessions, disk);
    view.type(0, "x");
    let called = false;
    const renaming = lease.rename(async (file) => {
      called = true;
      return file;
    });
    disk.writes[0].done.reject(new Error("nope"));
    expect(await renaming).toBe(false);
    expect(called).toBe(false);
  });
});
