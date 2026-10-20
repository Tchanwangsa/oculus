import { applyCanvasTransaction, type CanvasTransaction } from "@/lib/canvas/collaboration";
import { createCanvasDocument, serializeCanvasDocument, type CanvasDocument } from "@/lib/canvas/model";
import type { ShareClientEnv, ShareLifecycleEvent, StorageLike, DraftStorage, WebSocketLike } from "@/lib/companion/env";
import type { ShareChange } from "@/lib/companion/protocol";

/** Wire protocol v1 in process, applying commits with the real `collaboration.ts`
 * semantics (the authority's), so the share client runs against it end to end. */

export const flush = async (rounds = 12) => { for (let i = 0; i < rounds; i++) await new Promise((resolve) => setTimeout(resolve, 0)); };

export class FakeClock {
  time = 1_000_000;
  private timers: { id: number; at: number; fn: () => void }[] = [];
  private next = 1;
  now = () => this.time;
  setTimeout = (fn: () => void, ms: number) => { const id = this.next++; this.timers.push({ id, at: this.time + Math.max(0, ms), fn }); return id; };
  clearTimeout = (id: unknown) => { this.timers = this.timers.filter((timer) => timer.id !== id); };
  pending() { return this.timers.map((timer) => timer.at - this.time).sort((a, b) => a - b); }
  /** Runs due timers in order, letting promises settle after each. */
  async advance(ms: number) {
    const target = this.time + ms;
    for (;;) {
      const due = this.timers.filter((timer) => timer.at <= target).sort((a, b) => a.at - b.at || a.id - b.id)[0];
      if (!due) break;
      this.timers = this.timers.filter((timer) => timer !== due);
      this.time = due.at;
      due.fn();
      await flush();
    }
    this.time = target;
    await flush();
  }
}

export class MemoryStorage implements StorageLike {
  readonly map = new Map<string, string>();
  getItem(key: string) { return this.map.get(key) ?? null; }
  setItem(key: string, value: string) { this.map.set(key, value); }
  removeItem(key: string) { this.map.delete(key); }
}

/** A browser that refuses storage: every access throws. */
export const throwingStorage: StorageLike = {
  getItem() { throw new Error("SecurityError"); },
  setItem() { throw new Error("SecurityError"); },
  removeItem() { throw new Error("SecurityError"); },
};

export class FakeSocket implements WebSocketLike {
  onopen: WebSocketLike["onopen"] = null;
  onmessage: WebSocketLike["onmessage"] = null;
  onclose: WebSocketLike["onclose"] = null;
  onerror: WebSocketLike["onerror"] = null;
  readonly sent: string[] = [];
  clientId: string | null = null;
  closed = false;
  /** Frames are swallowed: a socket that looks open but hears nothing. */
  muted = false;
  constructor(private readonly server: FakeShareServer, readonly url: string) {}
  send(data: string) {
    if (this.closed) return;
    this.sent.push(data);
    this.server.onClientFrame(this, data);
  }
  close() { this.serverClose(); }
  deliver(frame: unknown) {
    if (this.closed || this.muted) return;
    const data = JSON.stringify(frame);
    queueMicrotask(() => { if (!this.closed) this.onmessage?.({ data } as MessageEvent); });
  }
  serverClose() {
    if (this.closed) return;
    this.closed = true;
    this.server.sockets.delete(this);
    queueMicrotask(() => this.onclose?.({} as CloseEvent));
  }
}

type Receipt = { opId: string; revision: number };
let ids = 0;
const json = (status: number, body: unknown) => new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
const conflictCode = (message: string) => message.includes("layer") ? "conflict-order" : message.includes("background") ? "conflict-background" : "conflict-shape";

export class FakeShareServer {
  readonly origin = "http://192.168.1.20:43821";
  readonly shareId = "0123456789abcdef0123456789abcdef";
  readonly secret = "S3cretValue_abcdefghijklmnopqrstuvwxyz012345";
  epoch = "epoch-1";
  revision = 0;
  doc: CanvasDocument;
  log: ShareChange[] = [];
  logLimit = 256;
  readonly sockets = new Set<FakeSocket>();
  readonly allSockets: FakeSocket[] = [];
  readonly requests: { method: string; path: string; body?: string; authorization: string | null }[] = [];
  private receipts = new Map<string, Map<number, Receipt>>();
  /** Requests and socket opens fail as if the Mac were unreachable. */
  down = false;
  /** Applies the next commit, then loses its response. */
  loseNextAck = false;
  /** Answers the next commit `stale-op`. */
  staleNext = false;
  paused = false;
  stopped = false;
  conflictMessage: string | null = null;
  /** Inline transactions up to this size in `change` frames (the plan's 32 KiB). */
  inlineLimit = 32 * 1024;

  constructor(doc: CanvasDocument = createCanvasDocument()) { this.doc = doc; }

  env(overrides: Partial<ShareClientEnv> & { clock: FakeClock; storage?: DraftStorage | null }): ShareClientEnv & { emit(event: ShareLifecycleEvent): void } {
    const listeners = new Set<(event: ShareLifecycleEvent) => void>();
    return {
      origin: this.origin,
      fetch: (input, init) => this.fetch(input, init),
      openSocket: (url) => this.openSocket(url),
      random: () => 0,
      newId: () => `id-${++ids}`,
      storage: null,
      lifecycle: (listener) => { listeners.add(listener); return () => listeners.delete(listener); },
      emit: (event) => { for (const listener of listeners) listener(event); },
      ...overrides,
    };
  }

  commits() { return this.requests.filter((request) => request.path.endsWith("/commit")).map((request) => JSON.parse(request.body!)); }

  /** A change from someone else (the desktop pane), broadcast like any commit. */
  remote(transaction: CanvasTransaction, clientId = "desktop") {
    this.doc = applyCanvasTransaction(this.doc, transaction);
    this.record(clientId, transaction);
  }

  /** App restart or Stop then Share again: a new epoch, every socket closed, no receipts. */
  restart(doc: CanvasDocument = this.doc) {
    this.epoch = `${this.epoch}+`;
    this.revision = 0;
    this.doc = doc;
    this.log = [];
    this.receipts.clear();
    for (const socket of [...this.sockets]) socket.serverClose();
  }

  /** Stop sharing: the grant goes, sockets hear `session-ended`. */
  stop() {
    this.stopped = true;
    for (const socket of [...this.sockets]) { socket.deliver({ type: "session-ended", reason: "stopped" }); queueMicrotask(() => socket.serverClose()); }
  }

  broadcast(frame: unknown) { for (const socket of this.sockets) socket.deliver(frame); }

  private record(clientId: string | null, transaction: CanvasTransaction) {
    this.revision += 1;
    const change: ShareChange = { epoch: this.epoch, revision: this.revision, clientId, transaction };
    this.log.push(change);
    if (this.log.length > this.logLimit) this.log.shift();
    const inline = JSON.stringify(transaction).length <= this.inlineLimit;
    this.broadcast(inline ? { type: "change", ...change } : { type: "revision", epoch: this.epoch, revision: this.revision });
  }

  openSocket(url: string): FakeSocket {
    const socket = new FakeSocket(this, url);
    this.allSockets.push(socket);
    queueMicrotask(() => {
      if (this.down || this.stopped) { socket.serverClose(); return; }
      this.sockets.add(socket);
      socket.onopen?.({} as Event);
    });
    return socket;
  }

  onClientFrame(socket: FakeSocket, data: string) {
    if (data.length > 64 * 1024) { socket.serverClose(); return; }
    const frame = JSON.parse(data);
    if (frame.type !== "hello") return;
    if (frame.secret !== this.secret) { socket.serverClose(); return; }
    socket.clientId = frame.clientId;
    socket.deliver({ type: "welcome", epoch: this.epoch, revision: this.revision, clients: this.sockets.size });
    this.broadcast({ type: "presence", clients: this.sockets.size });
  }

  async fetch(input: string, init: RequestInit = {}): Promise<Response> {
    await Promise.resolve();
    const url = new URL(input);
    const headers = new Headers(init.headers);
    const request = { method: init.method ?? "GET", path: url.pathname + url.search, body: typeof init.body === "string" ? init.body : undefined, authorization: headers.get("Authorization") };
    this.requests.push(request);
    if (this.down) throw new TypeError("Load failed");
    if (init.signal?.aborted) throw new DOMException("The operation was aborted.", "AbortError");
    if (!url.pathname.startsWith(`/api/s/${this.shareId}/`)) return json(404, { code: "not-found", message: "No such share." });
    if (request.authorization !== `Bearer ${this.shareId}.${this.secret}`) return json(401, { code: "unauthorized", message: "Not paired." });
    if (this.stopped) return json(404, { code: "not-found", message: "No such share." });
    const route = url.pathname.slice(`/api/s/${this.shareId}`.length);
    if (route === "/snapshot") {
      const text = serializeCanvasDocument(this.doc);
      return new Response(text, { status: 200, headers: { "Content-Type": "text/plain", "X-Oculus-Epoch": this.epoch, "X-Oculus-Revision": String(this.revision), "Content-Length": String(new TextEncoder().encode(text).byteLength) } });
    }
    if (route === "/changes") {
      const from = Number(url.searchParams.get("from"));
      const oldest = this.log[0]?.revision ?? this.revision + 1;
      if (from < oldest - 1 || from > this.revision) return json(410, { code: "gone", message: "Too old." });
      return json(200, this.log.filter((change) => change.revision > from));
    }
    if (route === "/commit" && request.method === "POST") return this.commit(JSON.parse(request.body!));
    return json(404, { code: "not-found", message: "No such route." });
  }

  private commit(op: { clientId: string; seq: number; opId: string; epoch: string; transaction: CanvasTransaction }): Response {
    if (op.epoch !== this.epoch) return json(410, { code: "session-ended", message: "This canvas is no longer shared." });
    if (this.paused) return json(423, { code: "external-edit", message: "The canvas file changed outside Oculus." });
    if (this.staleNext) { this.staleNext = false; return json(409, { code: "stale-op", message: "This change is too old to confirm." }); }
    const receipts = this.receipts.get(op.clientId) ?? new Map<number, Receipt>();
    const replay = receipts.get(op.seq);
    if (replay) return json(200, { epoch: this.epoch, revision: replay.revision, seq: op.seq, opId: replay.opId, replayed: true, transaction: null });
    if (receipts.size >= 64 && op.seq < Math.min(...receipts.keys())) return json(409, { code: "stale-op", message: "This change is too old to confirm." });
    try { this.doc = applyCanvasTransaction(this.doc, op.transaction); }
    catch (error) {
      const message = (error as Error).message;
      return json(409, { code: conflictCode(message), message: this.conflictMessage ?? message });
    }
    this.record(`web:${op.clientId}`, op.transaction);
    receipts.set(op.seq, { opId: op.opId, revision: this.revision });
    if (receipts.size > 64) receipts.delete(Math.min(...receipts.keys()));
    this.receipts.set(op.clientId, receipts);
    if (this.loseNextAck) { this.loseNextAck = false; throw new TypeError("Load failed"); }
    return json(200, { epoch: this.epoch, revision: this.revision, seq: op.seq, opId: op.opId, replayed: false, transaction: op.transaction });
  }
}
