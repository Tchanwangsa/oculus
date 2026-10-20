import { afterEach, beforeEach, describe, expect, spyOn, test } from "bun:test";
import { createCanvasDocument, parseCanvasDocument, type CanvasDocument, type CanvasElement } from "@/lib/canvas/model";
import { ShareApi } from "@/lib/companion/api";
import { BACKOFF_MAX_MS, backoffDelay, type DraftStorage } from "@/lib/companion/env";
import { draftKey } from "@/lib/companion/drafts";
import { parseShareFragment, takeShareCredentials } from "@/lib/companion/link";
import { ShareClient, type ShareClientOptions } from "@/lib/companion/shareClient";
import { FakeClock, FakeShareServer, flush, MemoryStorage, throwingStorage } from "./fakeShareServer";

const rect = (id: string, x = 0): CanvasElement => ({ id, type: "rectangle", x, y: 0, width: 10, height: 10, stroke: "#000000", fill: "transparent", strokeWidth: 1, style: "clean", seed: 1 });
const ids = (document: CanvasDocument | null | undefined) => document?.elements.map((el) => el.id) ?? [];
const withElements = (document: CanvasDocument, ...elements: CanvasElement[]) => ({ ...document, elements: [...document.elements, ...elements] });
const moved = (document: CanvasDocument, id: string, x: number) => ({ ...document, elements: document.elements.map((el) => el.id === id ? { ...el, x } : el) });

/** A PNG data URL just over `bytes` long: the PNG signature, then padding. */
function bigPng(bytes: number): string {
  const body = "iVBORw0KGgoAAAANSUhEUgAA" + "A".repeat(Math.ceil(bytes / 4) * 4);
  return `data:image/png;base64,${body}`;
}

let clock: FakeClock;
let server: FakeShareServer;
let clients: ShareClient[];

function client(storage: DraftStorage | null = new MemoryStorage(), options: ShareClientOptions = {}) {
  const env = server.env({ clock, storage });
  const shared = new ShareClient({ shareId: server.shareId, secret: server.secret }, env, options);
  clients.push(shared);
  return { client: shared, env, storage };
}

async function connected(storage?: DraftStorage | null, options?: ShareClientOptions) {
  const made = client(storage, options);
  made.client.start();
  await flush();
  expect(made.client.getState().status).toBe("connected");
  return made;
}

beforeEach(() => {
  clock = new FakeClock();
  server = new FakeShareServer();
  clients = [];
});
afterEach(() => { for (const shared of clients) shared.dispose(); });

describe("ShareClient", () => {
  for (const delivery of ["socket", "ack", "changes", "snapshot"] as const) {
    test(`a delete immediately undone survives its own ${delivery} confirmation`, async () => {
      const image: CanvasElement = { ...rect("page"), type: "image", dataUrl: bigPng(100), naturalWidth: 1, naturalHeight: 1 };
      server.doc = withElements(createCanvasDocument(), image, rect("annotation"));
      const { client: a } = await connected();
      const original = a.getState().document!;
      const socket = server.allSockets[0];
      if (delivery === "ack") socket.muted = true;
      if (delivery === "changes" || delivery === "snapshot") server.inlineLimit = 0;
      if (delivery === "snapshot") server.logLimit = 0;
      let send!: () => void, reply!: () => void;
      const sending = new Promise<void>(resolve => { send = resolve; });
      const replying = new Promise<void>(resolve => { reply = resolve; });
      const realFetch = server.fetch.bind(server);
      let first = true;
      server.fetch = async (input, init) => {
        if (!input.endsWith("/commit") || !first) return realFetch(input, init);
        first = false;
        await sending;
        const response = await realFetch(input, init);
        await replying;
        return response;
      };
      a.edit({ ...original, elements: original.elements.filter(el => el.id !== "page") });
      await flush();
      a.edit(original); // Undo before the delete has reached the Mac.
      await flush();
      send(); await flush();
      expect(a.getState().document).toBe(original);
      expect(a.getState().status).toBe("saving");
      reply(); await flush();
      expect(a.getState()).toMatchObject({ status: "connected", hasDraft: false, pendingOps: 0 });
      expect(a.getState().document!.elements).toEqual(original.elements);
      expect(server.doc.elements).toEqual(original.elements);
      expect(server.revision).toBe(2);
      expect(server.commits()).toHaveLength(2);
    });
  }

  test("successive edits to the same shape do not conflict with our own echo", async () => {
    server.doc = withElements(createCanvasDocument(), rect("x"));
    const { client: a } = await connected();
    let send!: () => void;
    const gate = new Promise<void>(resolve => { send = resolve; });
    const realFetch = server.fetch.bind(server);
    let first = true;
    server.fetch = async (input, init) => {
      if (input.endsWith("/commit") && first) { first = false; await gate; }
      return realFetch(input, init);
    };
    a.edit(moved(a.getState().document!, "x", 10));
    await flush();
    a.edit(moved(a.getState().document!, "x", 20));
    const latest = moved(a.getState().document!, "x", 30);
    a.edit(latest);
    send(); await flush();
    expect(a.getState().document).toBe(latest);
    expect(a.getState()).toMatchObject({ status: "connected", lastError: null });
    expect(server.doc.elements[0].x).toBe(30);
    expect(server.commits()).toHaveLength(2);
  });

  test("a quick undo survives a lost reply, recovery reload and operation replay", async () => {
    server.doc = withElements(createCanvasDocument(), rect("x"));
    const storage = new MemoryStorage();
    const { client: a } = await connected(storage);
    let send!: () => void;
    const gate = new Promise<void>(resolve => { send = resolve; });
    const realFetch = server.fetch.bind(server);
    let first = true;
    server.fetch = async (input, init) => {
      if (input.endsWith("/commit") && first) { first = false; await gate; }
      return realFetch(input, init);
    };
    const original = a.getState().document!;
    server.loseNextAck = true;
    a.edit({ ...original, elements: [] }); await flush();
    a.edit(original);
    send(); await flush();
    expect(a.getState().document).toBe(original);
    a.dispose(); await flush();
    const record = JSON.parse(storage.getItem(draftKey(server.shareId))!);
    expect(record.afterInflight.edits[0]).toMatchObject({ id: "x", before: null, after: rect("x") });
    const { client: restored } = await connected(storage);
    expect(restored.getState().document!.elements).toEqual(original.elements);
    expect(server.doc.elements).toEqual(original.elements);
    expect(server.revision).toBe(2);
    const [sent, replay] = server.commits();
    expect(replay.opId).toBe(sent.opId);
  });

  test("two clients editing at once converge on the server's drawing", async () => {
    const { client: a } = await connected();
    const { client: b } = await connected();
    expect(a.getState().clients).toBe(2);
    a.edit(withElements(a.getState().document!, rect("a")));
    b.edit(withElements(b.getState().document!, rect("b")));
    await flush();
    a.edit(moved(a.getState().document!, "b", 40));
    await flush();
    for (const shared of [a, b]) {
      expect(ids(shared.getState().document)).toEqual(ids(server.doc));
      expect(shared.getState().document!.elements).toEqual(server.doc.elements);
      expect(shared.getState()).toMatchObject({ status: "connected", pendingOps: 0, hasDraft: false, revision: server.revision });
    }
    expect(server.doc.elements.find((el) => el.id === "b")!.x).toBe(40);
  });

  test("a lost ack is retried as the same op and applied once", async () => {
    const { client: a } = await connected();
    server.loseNextAck = true;
    a.edit(withElements(a.getState().document!, rect("a")));
    await flush();
    expect(a.getState().status).toBe("offline-unsaved");
    expect(a.getState().pendingOps).toBe(1);
    await clock.advance(700);
    const [first, retry] = server.commits();
    expect(retry).toBeDefined();
    expect({ clientId: retry.clientId, seq: retry.seq, opId: retry.opId }).toEqual({ clientId: first.clientId, seq: first.seq, opId: first.opId });
    expect(server.revision).toBe(1);
    expect(ids(server.doc)).toEqual(["a"]);
    expect(ids(a.getState().document)).toEqual(["a"]);
    expect(a.getState()).toMatchObject({ status: "connected", pendingOps: 0 });
    a.edit(withElements(a.getState().document!, rect("b")));
    await flush();
    expect(server.commits().at(-1).seq).toBe(first.seq + 1);
  });

  test("a commit that never answers times out and is resent unchanged", async () => {
    const { client: a } = await connected();
    const realFetch = server.fetch.bind(server);
    let hang = true;
    const sent: { opId: string }[] = [];
    server.fetch = (input, init) => {
      if (input.endsWith("/commit")) sent.push(JSON.parse(init!.body as string));
      return hang && input.endsWith("/commit")
        ? new Promise((_, reject) => init?.signal?.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError"))))
        : realFetch(input, init);
    };
    a.edit(withElements(a.getState().document!, rect("a")));
    await flush();
    expect(a.getState().status).toBe("saving");
    hang = false;
    await clock.advance(21_000);
    await clock.advance(1_000);
    expect(sent.length).toBe(2);
    expect(sent[1].opId).toBe(sent[0].opId);
    expect(ids(server.doc)).toEqual(["a"]);
  });

  test("a new epoch ends the share and keeps the draft without replaying it", async () => {
    const storage = new MemoryStorage();
    const { client: a } = await connected(storage);
    server.down = true;
    for (const socket of [...server.sockets]) socket.serverClose();
    await flush();
    a.edit(withElements(a.getState().document!, rect("draft")));
    await flush();
    expect(a.getState().status).toBe("offline-unsaved");
    server.restart();
    server.down = false;
    await clock.advance(BACKOFF_MAX_MS);
    expect(a.getState().status).toBe("ended");
    expect(a.getState().hasDraft).toBe(true);
    expect(ids(parseCanvasDocument(a.draftText()!))).toEqual(["draft"]);
    expect(ids(server.doc)).toEqual([]);
    expect(server.commits().every((op) => op.epoch !== server.epoch)).toBe(true);
  });

  test("stop sharing ends the page and leaves the draft downloadable", async () => {
    const { client: a } = await connected();
    server.paused = true;
    a.edit(withElements(a.getState().document!, rect("mine")));
    await flush();
    server.stop();
    await flush();
    expect(a.getState().status).toBe("ended");
    expect(a.getState().lastError).toContain("Sharing was stopped");
    expect(ids(parseCanvasDocument(a.draftText()!))).toEqual(["mine"]);
  });

  test("a dropped socket backs off 0.5 s to 8 s and retries at once on online, visible and pageshow", async () => {
    expect([0, 1, 2, 3, 4, 5].map((n) => backoffDelay(n, () => 0))).toEqual([500, 1000, 2000, 4000, 8000, 8000]);
    expect(backoffDelay(1, () => 0.999)).toBeLessThanOrEqual(1250);
    expect(backoffDelay(9, () => 0.999)).toBe(8000);
    const { client: a, env } = await connected();
    server.down = true;
    for (const socket of [...server.sockets]) socket.serverClose();
    await flush();
    const delays: number[] = [];
    for (let i = 0; i < 6; i++) {
      const state = a.getState();
      expect(state.status).toBe("reconnecting");
      delays.push(state.retryAt! - clock.now());
      await clock.advance(state.retryAt! - clock.now());
    }
    expect(delays).toEqual([500, 1000, 2000, 4000, 8000, 8000]);
    for (const event of ["online", "visible", "pageshow"] as const) {
      server.down = false;
      const before = server.allSockets.length;
      env.emit(event);
      await flush();
      expect(server.allSockets.length).toBe(before + 1);
      expect(a.getState().status).toBe("connected");
      server.down = true;
      for (const socket of [...server.sockets]) socket.serverClose();
      await flush();
    }
  });

  test("a change log that no longer reaches back means a fresh snapshot", async () => {
    const { client: a } = await connected();
    server.logLimit = 2;
    const [socket] = [...server.sockets];
    socket.muted = true;
    for (let i = 0; i < 5; i++) server.remote({ edits: [{ id: `r${i}`, before: null, after: rect(`r${i}`, i) }] });
    socket.muted = false;
    socket.deliver({ type: "revision", epoch: server.epoch, revision: server.revision });
    await flush();
    const paths = server.requests.map((request) => request.path.replace(`/api/s/${server.shareId}`, ""));
    expect(paths.slice(-2)).toEqual(["/changes?from=0", "/snapshot"]);
    expect(a.getState().revision).toBe(5);
    expect(ids(a.getState().document)).toEqual(ids(server.doc));
  });

  test("changes that arrive while an edit is in flight are rebased under it", async () => {
    const { client: a } = await connected();
    a.edit(withElements(a.getState().document!, rect("mine")));
    server.remote({ edits: [{ id: "theirs", before: null, after: rect("theirs") }] });
    await flush();
    expect(new Set(ids(a.getState().document))).toEqual(new Set(["mine", "theirs"]));
    expect(ids(a.getState().document)).toEqual(ids(server.doc));
  });

  test("a conflict keeps the draft until the person chooses the shared canvas", async () => {
    server.doc = withElements(createCanvasDocument(), rect("x"));
    const { client: a } = await connected();
    const { client: b } = await connected();
    const [socketA] = server.allSockets;
    socketA.muted = true;
    b.edit(moved(b.getState().document!, "x", 100));
    await flush();
    a.edit(moved(a.getState().document!, "x", 50));
    await flush();
    expect(a.getState().status).toBe("conflict");
    expect(a.getState().lastError).toContain("collaborator");
    expect(parseCanvasDocument(a.draftText()!).elements[0].x).toBe(50);
    a.useSharedCanvas();
    await flush();
    expect(a.getState().document!.elements[0].x).toBe(100);
    expect(a.getState()).toMatchObject({ status: "connected", hasDraft: false });
  });

  test("a kept conflict survives a reload without discarding its downloadable drawing", async () => {
    server.doc = withElements(createCanvasDocument(), rect("x"));
    const storage = new MemoryStorage();
    const { client: a } = await connected(storage);
    server.allSockets[0].muted = true;
    server.remote({ edits: [{ id: "x", before: rect("x"), after: rect("x", 100) }] });
    a.edit(moved(a.getState().document!, "x", 50)); await flush();
    expect(a.getState().status).toBe("conflict");
    a.dispose(); await flush();
    const { client: reloaded } = client(storage);
    reloaded.start(); await flush();
    expect(reloaded.getState().status).toBe("conflict");
    expect(parseCanvasDocument(reloaded.draftText()!).elements[0].x).toBe(50);
    expect(server.doc.elements[0].x).toBe(100);
    reloaded.useSharedCanvas(); await flush();
    expect(reloaded.getState().status).toBe("connected");
    expect(reloaded.getState().document!.elements[0].x).toBe(100);
  });

  test("an image over 2 MiB commits over HTTP; the socket only carries small frames", async () => {
    const { client: a } = await connected();
    const { client: b } = await connected();
    const dataUrl = bigPng(2.2 * 1024 * 1024);
    const image: CanvasElement = { ...rect("img"), type: "image", width: 400, height: 300, dataUrl, naturalWidth: 400, naturalHeight: 300, sourceName: "photo.png" };
    a.edit(withElements(a.getState().document!, image));
    await flush();
    const commit = server.requests.find((request) => request.path.endsWith("/commit"))!;
    expect(commit.body!.length).toBeGreaterThan(2 * 1024 * 1024);
    for (const socket of server.allSockets) for (const frame of socket.sent) expect(frame.length).toBeLessThan(64 * 1024);
    expect(server.requests.some((request) => request.path.includes("/changes?from=0"))).toBe(true);
    expect(b.getState().document!.elements[0].dataUrl).toBe(dataUrl);
    expect(a.getState().status).toBe("connected");
  });

  test("a PDF-sized offline draft survives async storage and reload without a size warning", async () => {
    const memory = new MemoryStorage();
    const storage: DraftStorage = {
      getItem: async key => memory.getItem(key),
      setItem: async (key, value) => { await Promise.resolve(); memory.setItem(key, value); },
      removeItem: async key => memory.removeItem(key),
    };
    const { client: a } = await connected(storage);
    server.down = true;
    for (const socket of [...server.sockets]) socket.serverClose();
    await flush();
    const dataUrl = bigPng(6 * 1024 * 1024);
    a.edit(withElements(a.getState().document!, { ...rect("img"), type: "image", dataUrl, naturalWidth: 1, naturalHeight: 1 }));
    await clock.advance(450);
    expect(a.getState().storageWarning).toBeNull();
    const stored = JSON.parse(memory.getItem(draftKey(server.shareId))!);
    expect(parseCanvasDocument(stored.draft).elements[0].dataUrl).toBe(dataUrl);
    expect(stored.pending.edits).toHaveLength(1);
    a.dispose(); await flush();
    server.down = false;
    const { client: reloaded } = await connected(storage);
    expect(reloaded.getState().document!.elements[0].dataUrl).toBe(dataUrl);
    expect(server.doc.elements[0].dataUrl).toBe(dataUrl);
  });

  test("the in-flight identity reaches recovery storage before the commit is sent", async () => {
    const memory = new MemoryStorage();
    let release!: () => void;
    const blocked = new Promise<void>(resolve => { release = resolve; });
    let hold = false;
    const { client: a } = await connected({
      getItem: async key => memory.getItem(key),
      setItem: async (key, value) => { if (hold) await blocked; memory.setItem(key, value); },
      removeItem: async key => memory.removeItem(key),
    });
    hold = true;
    a.edit(withElements(a.getState().document!, rect('durable')));
    await flush();
    expect(server.commits()).toHaveLength(0);
    release(); await flush();
    expect(server.commits()).toHaveLength(1);
    expect(a.getState().status).toBe('connected');
    expect(ids(server.doc)).toEqual(['durable']);
  });

  test("the socket is presumed dead after 45 s without a frame; a ping keeps it alive", async () => {
    const { client: a } = await connected();
    const [socket] = [...server.sockets];
    await clock.advance(40_000);
    socket.deliver({ type: "ping" });
    await flush();
    await clock.advance(10_000);
    expect(socket.closed).toBe(false);
    expect(socket.sent.map((frame) => JSON.parse(frame).type)).toEqual(["hello", "pong"]);
    socket.muted = true;
    await clock.advance(45_000);
    expect(socket.closed).toBe(true);
    await clock.advance(1_000);
    expect(server.allSockets.length).toBe(2);
    expect(a.getState().status).toBe("connected");
  });

  test("works when storage throws, and says a reload would lose unsaved edits", async () => {
    const { client: a } = await connected(throwingStorage);
    a.edit(withElements(a.getState().document!, rect("a")));
    await flush();
    expect(ids(server.doc)).toEqual(["a"]);
    server.down = true;
    for (const socket of [...server.sockets]) socket.serverClose();
    await flush();
    a.edit(withElements(a.getState().document!, rect("b")));
    await clock.advance(450);
    expect(a.getState().status).toBe("offline-unsaved");
    expect(a.getState().storageWarning).toContain("not keeping a copy");
    server.down = false;
    const { client: none } = await connected(null);
    expect(none.getState().storageWarning).toBeNull();
  });

  test("pending edits survive a reload in the same epoch and keep their identity", async () => {
    const storage = new MemoryStorage();
    const { client: a } = await connected(storage);
    server.loseNextAck = true;
    a.edit(withElements(a.getState().document!, rect("sent")));
    await flush();
    server.down = true;
    a.edit(withElements(a.getState().document!, rect("unsent")));
    await clock.advance(450);
    a.dispose();
    const lost = server.commits()[0];
    server.down = false;
    const { client: reloaded } = client(storage);
    reloaded.start();
    await flush();
    const after = server.commits().slice(1);
    expect(after[0]).toMatchObject({ clientId: lost.clientId, seq: lost.seq, opId: lost.opId });
    expect(after[1]).toMatchObject({ clientId: lost.clientId, seq: lost.seq + 1 });
    expect(ids(server.doc)).toEqual(["sent", "unsent"]);
    expect(ids(reloaded.getState().document)).toEqual(["sent", "unsent"]);
    expect(reloaded.getState().status).toBe("connected");
  });

  test("a draft from another epoch is offered for download, never replayed", async () => {
    const storage = new MemoryStorage();
    const { client: a } = await connected(storage);
    server.down = true;
    for (const socket of [...server.sockets]) socket.serverClose();
    await flush();
    a.edit(withElements(a.getState().document!, rect("old")));
    await clock.advance(450);
    a.dispose();
    server.restart();
    server.down = false;
    const { client: reloaded } = client(storage);
    reloaded.start();
    await flush();
    expect(reloaded.getState().status).toBe("connected");
    expect(ids(reloaded.getState().document)).toEqual([]);
    expect(ids(server.doc)).toEqual([]);
    expect(server.commits()).toEqual([]);
    expect(reloaded.getState().recovery).not.toBeNull();
    expect(ids(parseCanvasDocument(reloaded.recoveryText()!))).toEqual(["old"]);
    reloaded.dismissRecovery();
    expect(reloaded.getState().recovery).toBeNull();
  });

  test("stale-op starts a fresh client identity and resends the draft", async () => {
    const { client: a } = await connected();
    server.staleNext = true;
    a.edit(withElements(a.getState().document!, rect("a")));
    await flush();
    const [refused, resent] = server.commits();
    expect(resent.clientId).not.toBe(refused.clientId);
    expect(resent.seq).toBe(1);
    expect(ids(server.doc)).toEqual(["a"]);
    expect(a.getState().status).toBe("connected");
  });

  test("an edit outside Oculus pauses saving until the Mac resumes", async () => {
    const { client: a } = await connected();
    server.paused = true;
    a.edit(withElements(a.getState().document!, rect("a")));
    await flush();
    expect(a.getState().status).toBe("paused");
    expect(a.getState().hasDraft).toBe(true);
    server.paused = false;
    server.broadcast({ type: "state", phase: "live" });
    await flush();
    expect(ids(server.doc)).toEqual(["a"]);
    expect(a.getState().status).toBe("connected");
    server.paused = true;
    a.edit(withElements(a.getState().document!, rect("b")));
    await flush();
    server.paused = false;
    a.retry();
    await flush();
    expect(ids(server.doc)).toEqual(["a", "b"]);
  });

  test("the secret never appears in an error, a state or the console", async () => {
    const spies = (["log", "info", "warn", "error", "debug"] as const).map((level) => spyOn(console, level));
    try {
      server.conflictMessage = `refused for ${server.secret}`;
      server.doc = withElements(createCanvasDocument(), rect("x"));
      const { client: a } = await connected();
      server.remote({ edits: [{ id: "x", before: rect("x"), after: rect("x", 9) }] });
      [...server.sockets][0].muted = true;
      server.doc = { ...server.doc };
      a.edit(moved(a.getState().document!, "x", 5));
      await flush();
      const seen: string[] = [JSON.stringify(a.getState())];
      server.down = true;
      for (const socket of [...server.sockets]) socket.serverClose();
      await clock.advance(600);
      seen.push(JSON.stringify(a.getState()));
      const api = new ShareApi(server.origin, server.shareId, server.secret, (input, init) => server.fetch(input, init));
      for (const call of [() => api.snapshot(), () => api.changes(0)]) {
        try { await call(); } catch (error) { seen.push(String(error), (error as Error).message); }
      }
      server.down = false;
      server.stop();
      await flush();
      seen.push(JSON.stringify(a.getState()));
      for (const spy of spies) for (const call of spy.mock.calls) seen.push(call.map(String).join(" "));
      expect(seen.some((text) => text.includes("•••") || text.includes("Load failed") || text.includes("Mac"))).toBe(true);
      for (const text of seen) expect(text).not.toContain(server.secret);
      for (const request of server.requests) expect(request.path).not.toContain(server.secret);
      for (const socket of server.allSockets) expect(socket.url).not.toContain(server.secret);
    } finally {
      for (const spy of spies) spy.mockRestore();
    }
  });
});

describe("share links", () => {
  const id = "0123456789abcdef0123456789abcdef";
  const secret = "S3cretValue_abcdefghijklmnopqrstuvwxyz012345";

  test("the fragment is read, stripped from the address and kept for this tab only", () => {
    const session = new MemoryStorage();
    const replaced: string[] = [];
    const page = (hash: string) => ({
      location: { hash, pathname: "/c/", search: "" },
      history: { state: null, replaceState: (_state: unknown, _title: string, url?: string | URL | null) => { replaced.push(String(url)); } },
      session,
    });
    expect(takeShareCredentials(page(`#${id}.${secret}`))).toEqual({ shareId: id, secret });
    expect(replaced).toEqual(["/c/"]);
    expect(takeShareCredentials(page(""))).toEqual({ shareId: id, secret });
    expect(replaced).toEqual(["/c/"]);
    takeShareCredentials(page("#not-a-link"));
    expect(replaced).toEqual(["/c/", "/c/"]);
    expect(takeShareCredentials({ ...page(""), session: throwingStorage as Storage })).toBeNull();
  });

  test("only a well-formed share id and secret are accepted", () => {
    expect(parseShareFragment(`#${id}.${secret}`)).toEqual({ shareId: id, secret });
    expect(parseShareFragment(`#${id}`)).toBeNull();
    expect(parseShareFragment(`#${id.toUpperCase()}.${secret}`)).toBeNull();
    expect(parseShareFragment(`#${id}.short`)).toBeNull();
  });
});
