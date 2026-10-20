import { describe, expect, test } from "bun:test";
import {
  CanvasAuthorityError,
  type CanvasAuthorityAck, type CanvasAuthorityEvent, type CanvasAuthorityIO, type CanvasAuthorityOp, type CanvasAuthoritySnapshot,
} from "@/lib/canvas/authority";
import { applyCanvasTransaction, diffCanvasContent, hasCanvasChanges, sameValue } from "@/lib/canvas/collaboration";
import { createCanvasDocument, parseCanvasDocument, serializeCanvasDocument, type CanvasDocument, type CanvasElement, type CanvasViewport } from "@/lib/canvas/model";
import { createCanvasSessions, type CanvasFileRef } from "@/lib/canvas/sessions";

const PATH = "courses/X/canvases/Ideas.canvas";
const FILE: CanvasFileRef = { id: 1, relative_path: PATH };
const tick = () => new Promise((resolve) => setTimeout(resolve, 0));
const settle = async () => { for (let i = 0; i < 10; i++) await tick(); };
function deferred() { let resolve!: () => void; const promise = new Promise<void>((r) => { resolve = r; }); return { promise, resolve }; }

const rect = (id: string, x = 0): CanvasElement => ({ id, type: "rectangle", x, y: 0, width: 10, height: 10, stroke: "#000000", fill: "transparent", strokeWidth: 1, style: "clean", seed: 1 });
const canvas = (elements: CanvasElement[], viewport: CanvasViewport = { x: 0, y: 0, zoom: 1 }) => serializeCanvasDocument({ ...createCanvasDocument(), elements, viewport });
const ids = (text: string | null | undefined) => parseCanvasDocument(text!).elements.map((el) => el.id);
const xOf = (text: string | null | undefined, id: string) => parseCanvasDocument(text!).elements.find((el) => el.id === id)?.x;
const conflictCode = (message: string) => message.includes("layer") ? "conflict-order" as const : message.includes("background") ? "conflict-background" as const : "conflict-shape" as const;

/** The native authority's semantics (`authority/session.rs`) over an in-memory disk. */
function fakeAuthority(disk: Map<string, string>) {
  type Live = {
    doc: CanvasDocument; revision: number; epoch: string; phase: "live" | "paused" | "ended"; rel: string; seen: string;
    receipts: Map<string, { last: number; recent: Map<number, { opId: string; revision: number }> }>; viewportPending: boolean;
  };
  let live: Live | null = null;
  let epochs = 0;
  const listeners: { fileId: number; fn: (event: CanvasAuthorityEvent) => void }[] = [];
  const queue: { fn: (event: CanvasAuthorityEvent) => void; event: CanvasAuthorityEvent }[] = [];
  const ops: CanvasAuthorityOp[] = [];
  const calls: string[] = [];
  const flags = { loseNextAck: false, staleNext: false, openGate: undefined as Promise<void> | undefined, snapshotGate: undefined as Promise<void> | undefined };
  const emit = (event: CanvasAuthorityEvent) => { for (const l of listeners) if (l.fileId === event.fileId) queue.push({ fn: l.fn, event }); };
  const state = (s: Live, phase: Live["phase"]) => { if (s.phase === phase) return; s.phase = phase; emit({ kind: "state", fileId: 1, epoch: s.epoch, phase }); };
  const snap = (s: Live): CanvasAuthoritySnapshot => ({ epoch: s.epoch, revision: s.revision, text: serializeCanvasDocument(s.doc), relativePath: s.rel, paused: s.phase === "paused" });
  const current = () => { if (!live || live.phase === "ended") throw new CanvasAuthorityError("session-ended", "This canvas is no longer shared."); return live; };
  const persist = (s: Live, text: string) => {
    if (!disk.has(s.rel)) { state(s, "ended"); throw new CanvasAuthorityError("session-ended", "The canvas file was removed."); }
    if (disk.get(s.rel) !== s.seen) { state(s, "paused"); throw new CanvasAuthorityError("external-edit", "The canvas file changed outside Oculus. Choose which version to keep."); }
    disk.set(s.rel, text); s.seen = text;
  };
  const io: CanvasAuthorityIO<CanvasFileRef> = {
    async open(file, text) {
      calls.push("open");
      if (flags.openGate) await flags.openGate;
      if (live && live.phase !== "ended") return snap(live);
      if (disk.get(file.relative_path) !== text) throw new CanvasAuthorityError("external-edit", "The canvas file changed outside Oculus.");
      live = { doc: parseCanvasDocument(text), revision: 0, epoch: `epoch-${++epochs}`, phase: "live", rel: file.relative_path, seen: text, receipts: new Map(), viewportPending: false };
      return snap(live);
    },
    async commit(_file, op) {
      calls.push("commit");
      ops.push(structuredClone(op));
      const s = current();
      if (s.phase === "paused") throw new CanvasAuthorityError("external-edit", "The canvas file changed outside Oculus.");
      if (op.epoch && op.epoch !== s.epoch) throw new CanvasAuthorityError("session-ended", "This canvas is no longer shared.");
      if (flags.staleNext) { flags.staleNext = false; throw new CanvasAuthorityError("stale-op", "This change is too old to confirm."); }
      const log = s.receipts.get(op.clientId);
      if (log && op.seq <= log.last) {
        const receipt = log.recent.get(op.seq);
        if (receipt?.opId === op.opId) return { epoch: s.epoch, revision: receipt.revision, seq: op.seq, opId: op.opId, replayed: true, transaction: null };
        throw new CanvasAuthorityError(receipt ? "invalid" : "stale-op", "Operation number reused or too old.");
      }
      let next: CanvasDocument;
      try { next = applyCanvasTransaction(s.doc, op.transaction); }
      catch (error) { throw new CanvasAuthorityError(conflictCode(String(error)), (error as Error).message); }
      const effective = diffCanvasContent(s.doc, next);
      const record = () => { const entry = s.receipts.get(op.clientId) ?? { last: 0, recent: new Map() }; entry.last = op.seq; entry.recent.set(op.seq, { opId: op.opId, revision: s.revision }); s.receipts.set(op.clientId, entry); };
      const viewport = op.viewport && !sameValue(op.viewport, s.doc.viewport) ? op.viewport : undefined;
      if (!hasCanvasChanges(effective)) {
        if (viewport) { s.doc = { ...s.doc, viewport }; s.viewportPending = true; }
        record();
        return { epoch: s.epoch, revision: s.revision, seq: op.seq, opId: op.opId, replayed: false, transaction: null };
      }
      if (viewport) next = { ...next, viewport };
      persist(s, serializeCanvasDocument(next));
      s.doc = next; s.revision += 1; s.viewportPending = false; record();
      emit({ kind: "change", fileId: 1, epoch: s.epoch, revision: s.revision, clientId: op.clientId, transaction: effective });
      const ack: CanvasAuthorityAck = { epoch: s.epoch, revision: s.revision, seq: op.seq, opId: op.opId, replayed: false, transaction: effective };
      if (flags.loseNextAck) { flags.loseNextAck = false; throw new Error("The reply was lost."); }
      return ack;
    },
    async snapshot() { calls.push("snapshot"); const snapshot = snap(current()); if (flags.snapshotGate) await flags.snapshotGate; return snapshot; },
    async resolve(_file, choice) {
      calls.push(`resolve:${choice}`);
      const s = current();
      if (s.phase === "live") return snap(s);
      const text = disk.get(s.rel)!;
      if (choice === "adopt-disk") {
        const adopted = parseCanvasDocument(text);
        const change = diffCanvasContent(s.doc, adopted);
        s.doc = adopted; s.seen = text; state(s, "live");
        if (hasCanvasChanges(change)) { s.revision += 1; emit({ kind: "change", fileId: 1, epoch: s.epoch, revision: s.revision, clientId: null, transaction: change }); }
      } else {
        s.seen = text; persist(s, serializeCanvasDocument(s.doc)); state(s, "live");
      }
      return snap(s);
    },
    async rename(file, title) {
      calls.push("rename");
      const s = current();
      const moved = s.rel.replace(/[^/]+$/, `${title}.canvas`);
      disk.set(moved, disk.get(s.rel)!); disk.delete(s.rel); s.rel = moved;
      return { ...file, relative_path: moved };
    },
    async remove() { calls.push("remove"); const s = current(); disk.delete(s.rel); state(s, "ended"); },
    async close() {
      calls.push("close");
      if (!live || live.phase === "ended") return;
      if (live.phase === "live" && live.viewportPending) persist(live, serializeCanvasDocument(live.doc));
      state(live, "ended");
    },
    onRemote(fileId, fn) {
      const entry = { fileId, fn };
      listeners.push(entry);
      return () => { listeners.splice(listeners.indexOf(entry), 1); };
    },
  };
  return {
    io, ops, calls, flags,
    get doc() { return live?.doc; },
    get revision() { return live?.revision; },
    pending: () => queue.length,
    pump() { while (queue.length) { const { fn, event } = queue.shift()!; fn(event); } },
    drop(match: (event: CanvasAuthorityEvent) => boolean) { for (let i = queue.length - 1; i >= 0; i--) if (match(queue[i].event)) queue.splice(i, 1); },
    end() { if (live) state(live, "ended"); },
  };
}

function harness(elements: CanvasElement[] = [rect("a")]) {
  const disk = new Map([[PATH, canvas(elements)]]);
  const authority = fakeAuthority(disk);
  const client = (resume = false) => {
    const drafts = new Map<string, string>();
    const writes: string[] = [];
    const calls: string[] = [];
    const sessions = createCanvasSessions<CanvasFileRef>({
      drafts: { getItem: (key) => drafts.get(key) ?? null, setItem: (key, text) => { drafts.set(key, text); }, removeItem: (key) => { drafts.delete(key); } },
      delay: 60_000,
      read: async (path) => { if (!disk.has(path)) throw new Error("missing"); return disk.get(path)!; },
      write: async (ref, text) => { writes.push(text); disk.set(ref.relative_path, text); },
      rename: async (ref, title) => { calls.push("rename"); const moved = ref.relative_path.replace(/[^/]+$/, `${title}.canvas`); disk.set(moved, disk.get(ref.relative_path)!); disk.delete(ref.relative_path); return { ...ref, relative_path: moved }; },
      remove: async (ref) => { calls.push("remove"); disk.delete(ref.relative_path); },
      validate: (text) => { parseCanvasDocument(text); },
      authority: resume ? { ...authority.io, existing: async (file) => {
        try { return await authority.io.snapshot(file); }
        catch (error) { if (error instanceof CanvasAuthorityError && error.code === "session-ended") return null; throw error; }
      } } : authority.io,
    });
    const session = sessions.open(FILE);
    return { sessions, session, drafts, writes, calls, text: () => session.snapshot().text };
  };
  return { disk, authority, client, text: () => disk.get(PATH) };
}

async function shared(elements?: CanvasElement[]) {
  const h = harness(elements);
  const a = h.client(); const b = h.client();
  await a.session.load(); await b.session.load();
  await a.session.attachAuthority(); await b.session.attachAuthority();
  await settle();
  return { h, a, b };
}

describe("shared canvas sessions", () => {
  for (const eventFirst of [false, true]) {
    test(`an immediate undo survives a delayed desktop commit (${eventFirst ? "event" : "ack"} first)`, async () => {
      const image: CanvasElement = { ...rect("page"), type: "image", dataUrl: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a5XcAAAAASUVORK5CYII=", naturalWidth: 1, naturalHeight: 1 };
      const { h, a, b } = await shared([image, rect("ink")]);
      const gate = deferred();
      const commit = h.authority.io.commit;
      let first = true;
      h.authority.io.commit = async (file, op) => {
        const ack = await commit(file, op);
        if (first) { first = false; await gate.promise; }
        return ack;
      };
      const original = a.text()!;
      a.session.update(canvas([rect("ink")]));
      const saving = a.session.flush(); await tick();
      expect(ids(h.text())).toEqual(["ink"]);
      a.session.update(original);
      if (eventFirst) h.authority.pump();
      expect(a.text()).toBe(original);
      gate.resolve(); await saving; h.authority.pump();
      expect(a.text()).toBe(original);
      expect(ids(b.text())).toEqual(["page", "ink"]);
      expect(ids(h.text())).toEqual(["page", "ink"]);
      expect(a.session.snapshot().status).toBe("saved");
      expect(a.session.snapshot().conflict).toBeUndefined();
      expect(h.authority.revision).toBe(2);
    });
  }

  test("a second edit of the same shape while saving does not conflict with our own event", async () => {
    const { h, a } = await shared();
    const gate = deferred();
    const commit = h.authority.io.commit;
    let first = true;
    h.authority.io.commit = async (file, op) => {
      const ack = await commit(file, op);
      if (first) { first = false; await gate.promise; }
      return ack;
    };
    a.session.update(canvas([rect("a", 10)]));
    const saving = a.session.flush(); await tick();
    a.session.update(canvas([rect("a", 20)]));
    h.authority.pump();
    expect(a.session.snapshot().conflict).toBeUndefined();
    expect(xOf(a.text(), "a")).toBe(20);
    gate.resolve(); await saving;
    expect(xOf(h.text(), "a")).toBe(20);
    expect(a.session.snapshot().status).toBe("saved");
  });

  test("a fresh webview joins the surviving authority on load and receives edits without opening Share", async () => {
    const { h, a } = await shared();
    a.session.update(canvas([rect("a"), rect("before-reload")])); await a.session.flush();
    const restarted = h.client(true);
    let notifications = 0;
    restarted.session.subscribe(() => { notifications += 1; });
    await restarted.session.load();
    expect(restarted.session.authority).toBe("live");
    expect(ids(restarted.text())).toEqual(["a", "before-reload"]);
    const initial = notifications;
    a.session.update(canvas([rect("a"), rect("before-reload"), rect("remote")])); await a.session.flush();
    h.authority.pump();
    expect(notifications).toBeGreaterThan(initial);
    expect(ids(restarted.text())).toEqual(["a", "before-reload", "remote"]);
    restarted.session.update(canvas([rect("a"), rect("before-reload"), rect("remote"), rect("desktop")]));
    await restarted.session.flush(); h.authority.pump();
    expect(restarted.writes).toEqual([]);
    expect(ids(a.text())).toEqual(["a", "before-reload", "remote", "desktop"]);
    await restarted.session.attachAuthority();
    expect(restarted.writes).toEqual([]);
  });

  test("a native probe with no live authority leaves ordinary canvas saves available", async () => {
    const h = harness(); const a = h.client(true);
    await a.session.load();
    expect(a.session.authority).toBe("off");
    a.session.update(canvas([rect("a"), rect("ordinary")])); await a.session.flush();
    expect(a.writes.map(ids)).toEqual([["a", "ordinary"]]);
  });

  test("changes arriving while a fresh webview probes the native snapshot are replayed after it", async () => {
    const { h, a } = await shared();
    const gate = deferred(); h.authority.flags.snapshotGate = gate.promise;
    const restarted = h.client(true);
    const loading = restarted.session.load(); await tick();
    a.session.update(canvas([rect("a"), rect("during-probe")])); await a.session.flush();
    h.authority.pump();
    gate.resolve(); await loading;
    expect(restarted.session.authority).toBe("live");
    expect(ids(restarted.text())).toEqual(["a", "during-probe"]);
    expect(restarted.writes).toEqual([]);
    expect(h.authority.calls.filter((call) => call === "open")).toHaveLength(2);
  });

  test("a surviving paused share preserves the local recovery choice when the webview rejoins", async () => {
    const { h, a } = await shared();
    h.disk.set(PATH, canvas([rect("a"), rect("outside")]));
    a.session.update(canvas([rect("a"), rect("unsent")]));
    await expect(a.session.flush()).rejects.toThrow("outside Oculus");
    const restarted = h.client(true);
    restarted.drafts.set("oculus:canvas-draft:1", JSON.stringify({ version: 1, base: "shared:epoch-1", text: canvas([rect("a"), rect("draft")]) }));
    await restarted.session.load();
    expect(restarted.session.authority).toBe("paused");
    expect(ids(restarted.session.snapshot().recovery?.text)).toEqual(["a", "draft"]);
    expect(restarted.writes).toEqual([]);
  });

  test("a loaded desktop rejoins a later native owner before flushing its unsent edits", async () => {
    const h = harness(); const desktop = h.client(true); await desktop.session.load();
    desktop.session.update(canvas([rect("a"), rect("local")]));
    const other = h.client(); await other.session.load(); await other.session.attachAuthority();
    other.session.update(canvas([rect("a"), rect("remote")])); await other.session.flush();
    await desktop.session.attachAuthority(); await desktop.session.flush(); h.authority.pump();
    expect(desktop.session.authority).toBe("live");
    expect(ids(desktop.text())).toEqual(["a", "remote", "local"]);
    expect(ids(h.text())).toEqual(["a", "remote", "local"]);
    expect(desktop.writes).toEqual([]);
  });
  test("an edit made while the authority opens is committed, never written to the file", async () => {
    const h = harness(); const a = h.client(); await a.session.load();
    const gate = deferred(); h.authority.flags.openGate = gate.promise;
    a.session.update(canvas([rect("a"), rect("b")]));
    const attach = a.session.attachAuthority(); await tick();
    expect(a.writes.map(ids)).toEqual([["a", "b"]]);
    expect(a.session.authority).toBe("attaching");
    a.session.update(canvas([rect("a"), rect("b"), rect("c")]));
    const flush = a.session.flush(); await tick();
    expect(a.writes.length).toBe(1);
    gate.resolve(); await attach; await flush; await settle();
    expect(a.session.authority).toBe("live");
    expect(a.writes.length).toBe(1);
    expect(h.authority.ops.map((op) => op.transaction.edits.map((edit) => edit.id))).toEqual([["c"]]);
    expect(ids(h.text())).toEqual(["a", "b", "c"]);
    expect(a.session.snapshot().status).toBe("saved"); expect(a.drafts.size).toBe(0);
    a.session.update(canvas([rect("a"), rect("b"), rect("c"), rect("d")])); await a.session.flush();
    expect(a.writes.length).toBe(1); expect(ids(h.text())).toEqual(["a", "b", "c", "d"]);
  });

  test("independent edits from two clients merge; a remote change keeps unsent local edits", async () => {
    const { h, a, b } = await shared();
    expect(h.authority.calls.filter((c) => c === "open").length).toBe(2);
    a.session.update(canvas([rect("a"), rect("x")]));
    b.session.update(canvas([rect("a", 5), rect("y")]));
    await a.session.flush(); h.authority.pump();
    expect(ids(b.text())).toEqual(["a", "x", "y"]); expect(xOf(b.text(), "a")).toBe(5);
    expect(b.session.snapshot().status).toBe("unsaved");
    await tick();
    expect(ids(JSON.parse(b.drafts.values().next().value!).text)).toEqual(["a", "x", "y"]);
    await b.session.flush(); h.authority.pump();
    expect(ids(a.text())).toEqual(["a", "x", "y"]); expect(xOf(a.text(), "a")).toBe(5);
    expect(ids(h.text())).toEqual(["a", "x", "y"]); expect(h.authority.revision).toBe(2);
    expect(a.session.snapshot().status).toBe("saved"); expect(b.session.snapshot().status).toBe("saved");
    expect(a.writes.length + b.writes.length).toBe(0);
  });

  test("a commit refused for the same shape keeps the draft until the shared canvas is chosen", async () => {
    const { h, a, b } = await shared();
    a.session.update(canvas([rect("a", 50)])); b.session.update(canvas([rect("a", 90)]));
    await a.session.flush();
    await expect(b.session.flush()).rejects.toThrow("same shape");
    expect(b.session.snapshot().conflict).toContain("same shape");
    expect(b.session.snapshot().status).toBe("error"); expect(xOf(b.text(), "a")).toBe(90); expect(b.drafts.size).toBe(1);
    b.session.update(canvas([rect("a", 91)])); expect(xOf(b.text(), "a")).toBe(90);
    h.authority.pump(); expect(xOf(b.text(), "a")).toBe(90);
    await b.session.useSharedCanvas();
    expect(b.session.snapshot().conflict).toBeUndefined(); expect(b.session.snapshot().status).toBe("saved");
    expect(xOf(b.text(), "a")).toBe(50); expect(b.drafts.size).toBe(0);
    b.session.update(canvas([rect("a", 60)])); await b.session.flush();
    expect(xOf(h.text(), "a")).toBe(60);
  });

  test("a remote change to a shape with an unsent edit is a conflict found locally", async () => {
    const { h, a, b } = await shared();
    b.session.update(canvas([rect("a", 90)]));
    a.session.update(canvas([rect("a", 50)])); await a.session.flush();
    const sent = h.authority.ops.length;
    h.authority.pump();
    expect(b.session.snapshot().conflict).toContain("same shape"); expect(xOf(b.text(), "a")).toBe(90);
    await expect(b.session.flush()).rejects.toThrow("same shape");
    expect(h.authority.ops.length).toBe(sent);
  });

  test("a lost ack is retried with the same seq and opId and applied once", async () => {
    const { h, a } = await shared();
    h.authority.flags.loseNextAck = true;
    a.session.update(canvas([rect("a"), rect("x")]));
    await expect(a.session.flush()).rejects.toThrow("lost");
    expect(a.session.snapshot().status).toBe("error"); expect(a.drafts.size).toBe(1);
    await a.session.flush();
    const [first, retry] = h.authority.ops;
    expect(retry.seq).toBe(first.seq); expect(retry.opId).toBe(first.opId);
    expect(h.authority.revision).toBe(1); expect(ids(h.text())).toEqual(["a", "x"]);
    expect(a.session.snapshot().status).toBe("saved"); expect(a.drafts.size).toBe(0);
    h.authority.pump();
    a.session.update(canvas([rect("a"), rect("x"), rect("y")])); await a.session.flush();
    expect(h.authority.ops[2].seq).toBe(first.seq + 1);
    expect(ids(h.text())).toEqual(["a", "x", "y"]);
  });

  test("a revision gap refetches the snapshot and keeps unsent edits", async () => {
    const { h, a, b } = await shared();
    a.session.update(canvas([rect("a"), rect("mine")]));
    b.session.update(canvas([rect("a"), rect("x")])); await b.session.flush();
    b.session.update(canvas([rect("a"), rect("x"), rect("y")])); await b.session.flush();
    h.authority.drop((event) => event.kind === "change" && event.revision === 1);
    h.authority.pump(); await settle();
    expect(h.authority.calls.filter((c) => c === "snapshot").length).toBe(1);
    expect(ids(a.text())).toEqual(["a", "x", "y", "mine"]);
    await a.session.flush();
    expect(ids(h.text())).toEqual(["a", "x", "y", "mine"]);
  });

  test("a stale op resyncs under a fresh client id and keeps the draft", async () => {
    const { h, a } = await shared();
    h.authority.flags.staleNext = true;
    a.session.update(canvas([rect("a"), rect("x")])); await a.session.flush();
    const [stale, sent] = h.authority.ops;
    expect(sent.clientId).not.toBe(stale.clientId);
    expect(ids(h.text())).toEqual(["a", "x"]); expect(a.session.snapshot().status).toBe("saved");
  });

  test("a viewport change is a viewport-only commit that bumps nothing", async () => {
    const { h, a, b } = await shared();
    const before = b.text();
    a.session.update(canvas([rect("a")], { x: 40, y: 0, zoom: 2 })); await a.session.flush();
    const [op] = h.authority.ops;
    expect(op.transaction).toEqual({ edits: [] }); expect(op.viewport).toEqual({ x: 40, y: 0, zoom: 2 });
    expect(h.authority.revision).toBe(0); expect(h.authority.pending()).toBe(0);
    expect(b.text()).toBe(before); expect(a.session.snapshot().status).toBe("saved");
    await a.session.flush(); expect(h.authority.ops.length).toBe(1);
  });

  test("a pane mounted after attach joins the live session, which outlives its panes until detach", async () => {
    const h = harness(); const a = h.client(); await a.session.load(); await a.session.attachAuthority();
    a.session.release(); await settle();
    const pane = a.sessions.open(FILE);
    expect(pane).toBe(a.session);
    expect(pane.snapshot().authority).toBe("live"); expect(ids(pane.snapshot().text)).toEqual(["a"]);
    a.session.update(canvas([rect("a")], { x: 9, y: 0, zoom: 1 }));
    await a.session.detachAuthority();
    expect(a.session.authority).toBe("off"); expect(h.authority.calls).toContain("close");
    expect(parseCanvasDocument(h.text()!).viewport.x).toBe(9);
    expect(a.sessions.get(FILE.id)).toBeUndefined();
    const later = a.sessions.open(FILE); await later.load();
    later.update(canvas([rect("a"), rect("z")])); await later.flush();
    expect(ids(h.text())).toEqual(["a", "z"]);
  });

  test("an ended share falls back to the file; an unsent change becomes a recovery choice", async () => {
    const { h, a } = await shared();
    h.authority.end(); h.authority.pump(); await settle();
    expect(a.session.authority).toBe("ended"); expect(a.session.snapshot().status).toBe("saved");
    a.session.update(canvas([rect("a"), rect("z")])); await a.session.flush();
    expect(a.writes.map(ids)).toEqual([["a", "z"]]);

    const next = await shared();
    next.a.session.update(canvas([rect("a"), rect("unsent")]));
    next.h.authority.end(); next.h.authority.pump(); await settle();
    const snapshot = next.a.session.snapshot();
    expect(ids(snapshot.text)).toEqual(["a"]); expect(ids(snapshot.recovery?.text)).toEqual(["a", "unsent"]);
    expect(next.a.drafts.size).toBe(1);
    next.a.session.recoverDraft(); await next.a.session.flush();
    expect(ids(next.h.text())).toEqual(["a", "unsent"]);
  });

  test("an edit made while sharing ends is saved through the file", async () => {
    const { h, a } = await shared();
    h.authority.end(); h.authority.pump();
    a.session.update(canvas([rect("a"), rect("after")]));
    await settle(); await a.session.flush();
    expect(a.session.snapshot().recovery).toBeUndefined();
    expect(ids(h.text())).toEqual(["a", "after"]); expect(a.writes.map(ids)).toEqual([["a", "after"]]);
  });

  test("an outside edit pauses commits until the disk version is adopted or overwritten", async () => {
    const { h, a } = await shared();
    h.disk.set(PATH, canvas([rect("a"), rect("outside")]));
    a.session.update(canvas([rect("a"), rect("mine")]));
    await expect(a.session.flush()).rejects.toThrow("outside Oculus");
    h.authority.pump();
    expect(a.session.authority).toBe("paused"); expect(a.drafts.size).toBe(1);
    await a.session.resolveExternal("adopt-disk"); h.authority.pump();
    expect(a.session.authority).toBe("live"); expect(a.session.snapshot().status).toBe("saved");
    expect(ids(h.text())).toEqual(["a", "outside", "mine"]); expect(ids(a.text())).toEqual(["a", "outside", "mine"]);

    const kept = await shared();
    kept.h.disk.set(PATH, canvas([rect("a"), rect("outside")]));
    kept.a.session.update(canvas([rect("a"), rect("mine")]));
    await expect(kept.a.session.flush()).rejects.toThrow("outside Oculus");
    await kept.a.session.resolveExternal("keep-ours");
    expect(ids(kept.h.text())).toEqual(["a", "mine"]); expect(kept.a.session.snapshot().status).toBe("saved");
  });

  test("rename and delete go through the authority, and nothing writes after delete", async () => {
    const { h, a } = await shared();
    await a.session.rename("Plan");
    expect(a.calls).toEqual([]); expect(h.authority.calls).toContain("rename");
    expect(a.session.file.relative_path).toBe("courses/X/canvases/Plan.canvas");
    a.session.update(canvas([rect("a"), rect("p")])); await a.session.flush();
    expect(ids(h.disk.get("courses/X/canvases/Plan.canvas"))).toEqual(["a", "p"]); expect(a.writes.length).toBe(0);
    await a.session.remove();
    expect(a.calls).toEqual([]); expect(h.authority.calls).toContain("remove");
    expect(a.session.snapshot().status).toBe("deleted"); expect(a.session.authority).toBe("ended");
    a.session.update(canvas([rect("late")])); await a.session.flush(); h.authority.pump(); await settle();
    expect(h.disk.size).toBe(0); expect(a.writes.length).toBe(0); expect(a.drafts.size).toBe(0);
  });
});
