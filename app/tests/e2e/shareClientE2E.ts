/**
 * End-to-end check: real `ShareClient`s over bun's fetch, WebSocket and timers
 * against the real companion server. The Rust test `companion::tests::e2e`
 * starts the server, shares a canvas and runs this script with the port in
 * `OCULUS_E2E_PORT` and the link key in `OCULUS_E2E_KEY`; it plays the desktop
 * when asked: a line `@@ <command> <json>` on stdout is answered by
 * `@@ok <json>` (or `@@err <message>`) on stdin. Progress goes to stderr, and
 * neither stream may ever carry a secret. Not a `*.test.ts`, so `bun test`
 * skips it. See docs/device-sharing.md "End-to-end check".
 */
import { createHash } from "node:crypto";
import { createInterface } from "node:readline";
import { parseCanvasDocument, serializeCanvasDocument, type CanvasDocument, type CanvasElement } from "@/lib/canvas/model";
import { createCanvasSessions } from "@/lib/canvas/sessions";
import type { CanvasAuthorityEvent, CanvasAuthorityIO } from "@/lib/canvas/authority";
import type { FetchLike } from "@/lib/companion/api";
import { browserShareEnv, type SharePage, type StorageLike, type WebSocketLike } from "@/lib/companion/env";
import { draftKey } from "@/lib/companion/drafts";
import { parseShareFragment, type ShareCredentials } from "@/lib/companion/link";
import { ShareClient, type ShareClientState, type ShareStatus } from "@/lib/companion/shareClient";

const port = Number(process.env.OCULUS_E2E_PORT);
const firstKey = parseShareFragment(process.env.OCULUS_E2E_KEY ?? "");
if (!port || !firstKey) fail("OCULUS_E2E_PORT and OCULUS_E2E_KEY must be set (run through the Rust test).");

function fail(message: string): never {
  console.error(`[e2e] FAIL: ${message}`);
  process.exit(1);
}

function check(condition: unknown, message: string): asserts condition {
  if (!condition) fail(message);
}

const log = (message: string) => console.error(`[e2e] ${message}`);

/** The desktop side, answered by the Rust test over stdin. */
const replies: string[] = [];
const waiters: ((line: string) => void)[] = [];
const stdin = createInterface({ input: process.stdin });
stdin.on("line", (line) => { const waiter = waiters.shift(); if (waiter) waiter(line); else replies.push(line); });

async function desktop<T = unknown>(command: string, args: unknown = {}): Promise<T> {
  process.stdout.write(`@@ ${command} ${JSON.stringify(args)}\n`);
  const line = await within(30_000, `the desktop's answer to ${command}`, new Promise<string>((resolve) => {
    const queued = replies.shift();
    if (queued !== undefined) resolve(queued); else waiters.push(resolve);
  }));
  if (!line.startsWith("@@ok")) fail(`desktop ${command}: ${line}`);
  return JSON.parse(line.slice(4).trim() || "null") as T;
}

function within<T>(ms: number, what: string, promise: Promise<T>): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error(`timed out waiting for ${what}`)), ms); });
  return Promise.race([promise, timeout]).finally(() => clearTimeout(timer));
}

class MemoryStorage implements StorageLike {
  readonly map = new Map<string, string>();
  getItem(key: string) { return this.map.get(key) ?? null; }
  setItem(key: string, value: string) { this.map.set(key, value); }
  removeItem(key: string) { this.map.delete(key); }
}

interface CommitRecord { body: { clientId: string; seq: number; opId: string; epoch: string; transaction: { edits: { id: string }[] } }; status?: number; answer?: Record<string, unknown> | null; dropped?: boolean }

/**
 * One browser tab: its origin, storage and a `window` stand-in over bun's
 * globals, with hooks to lose a commit's answer or hold a commit back. Like a
 * browser, it sends `Origin` on POST and on the WebSocket upgrade.
 */
class Device {
  readonly commits: CommitRecord[] = [];
  readonly sockets: WebSocket[] = [];
  readonly statuses: ShareStatus[] = [];
  dropNextAnswer = false;
  /** Text `pong`s sent: the answers to the server's text pings. */
  pongs = 0;
  private hold: { promise: Promise<void>; release: () => void } | null = null;

  constructor(readonly name: string, readonly origin: string, readonly storage = new MemoryStorage()) {}

  holdCommits(): void {
    let release = () => {};
    const promise = new Promise<void>((resolve) => { release = resolve; });
    this.hold = { promise, release };
  }

  release(): void { this.hold?.release(); this.hold = null; }

  page(): SharePage {
    const device = this;
    const origin = this.origin;
    const fetchImpl: FetchLike = async (input, init = {}) => {
      const headers = new Headers(init.headers);
      const post = init.method === "POST";
      if (post) headers.set("Origin", origin);
      const request = { ...init, headers };
      if (!(post && input.endsWith("/commit"))) return fetch(input, request);
      const record: CommitRecord = { body: JSON.parse(String(init.body)) };
      device.commits.push(record);
      if (device.hold) await device.hold.promise;
      const response = await fetch(input, request);
      record.status = response.status;
      record.answer = await response.clone().json().catch(() => null);
      if (device.dropNextAnswer) {
        device.dropNextAnswer = false;
        record.dropped = true;
        // The request reached the Mac and was applied; only its answer is lost.
        throw new TypeError("Load failed");
      }
      return response;
    };
    class PageSocket extends WebSocket {
      constructor(url: string) {
        super(url, { headers: { Origin: origin } } as unknown as string[]);
        device.sockets.push(this);
      }
      override send(data: Parameters<WebSocket["send"]>[0]) {
        if (typeof data === "string" && data.includes('"pong"')) device.pongs++;
        super.send(data);
      }
    }
    const noListen = () => {};
    return {
      location: { origin },
      fetch: fetchImpl,
      WebSocket: PageSocket as unknown as new (url: string) => WebSocketLike,
      setTimeout: (callback, ms) => setTimeout(callback, ms),
      clearTimeout: (handle) => clearTimeout(handle as unknown as ReturnType<typeof setTimeout>),
      localStorage: this.storage,
      addEventListener: noListen,
      removeEventListener: noListen,
      document: { visibilityState: "visible", addEventListener: noListen, removeEventListener: noListen },
    };
  }

  client(credentials: ShareCredentials): ShareClient {
    // The server pings every 200 ms here (the Rust test's limits), so a dead
    // timer this short holds only while text pings arrive and are answered.
    const client = new ShareClient(credentials, { ...browserShareEnv(this.page()), storage: this.storage }, { deadManMs: 2_000 });
    client.subscribe(() => {
      const status = client.getState().status;
      if (this.statuses.at(-1) !== status) this.statuses.push(status);
    });
    return client;
  }
}

const rect = (id: string, x = 0): CanvasElement => ({ id, type: "rectangle", x, y: 0, width: 40, height: 30, stroke: "#000", fill: "transparent", strokeWidth: 2, style: "clean", seed: 1 });
const ids = (document: CanvasDocument | null | undefined) => document?.elements.map((el) => el.id) ?? [];
const find = (document: CanvasDocument | null | undefined, id: string) => document?.elements.find((el) => el.id === id);
const added = (client: ShareClient, element: CanvasElement) => {
  const document = client.getState().document!;
  return { ...document, elements: [...document.elements, element] };
};
const moved = (client: ShareClient, id: string, x: number) => {
  const document = client.getState().document!;
  return { ...document, elements: document.elements.map((el) => el.id === id ? { ...el, x } : el) };
};
const sameIds = (document: CanvasDocument | null, expected: string[]) => {
  const have = ids(document);
  return have.length === expected.length && expected.every((id) => have.includes(id));
};
const settled = (state: ShareClientState) => state.status === "connected" && state.pendingOps === 0;

function describe(state: ShareClientState): string {
  return JSON.stringify({ status: state.status, ids: ids(state.document), pendingOps: state.pendingOps, revision: state.revision, clients: state.clients, lastError: state.lastError });
}

/** Resolves once `test` holds for the client's state; event-driven, no polling. */
function until(client: ShareClient, label: string, test: (state: ShareClientState) => boolean, ms = 10_000): Promise<ShareClientState> {
  return new Promise((resolve) => {
    let off = () => {};
    const timer = setTimeout(() => { off(); fail(`${label}: timed out in state ${describe(client.getState())}`); }, ms);
    const probe = () => {
      const state = client.getState();
      if (!test(state)) return false;
      clearTimeout(timer);
      off();
      resolve(state);
      return true;
    };
    if (!probe()) off = client.subscribe(() => { probe(); });
  });
}

async function snapshot(origin: string, key: ShareCredentials) {
  const response = await fetch(`${origin}/api/s/${key.shareId}/snapshot`, { headers: { Authorization: `Bearer ${key.shareId}.${key.secret}` } });
  check(response.status === 200, `snapshot answered ${response.status}`);
  const text = await response.text();
  check(Number(response.headers.get("Content-Length")) === Buffer.byteLength(text), "the snapshot's Content-Length is exact");
  return { epoch: response.headers.get("X-Oculus-Epoch")!, revision: Number(response.headers.get("X-Oculus-Revision")), document: parseCanvasDocument(text), text };
}

function sameElements(a: ShareClient, b: ShareClient, what: string) {
  check(JSON.stringify(a.getState().document?.elements) === JSON.stringify(b.getState().document?.elements), `${what}: the two clients' drawings differ`);
}

const local = `http://127.0.0.1:${port}`;
const named = `http://localhost:${port}`;
const key = firstKey!;

// (a) Two tabs, one on 127.0.0.1 and one on localhost, load the snapshot and
// see each other's strokes.
const A = new Device("A", local);
const B = new Device("B", named);
const a = A.client(key);
const b = B.client(key);
a.start();
b.start();
await until(a, "A connects", (s) => settled(s) && s.clients === 2);
await until(b, "B connects", (s) => settled(s) && s.clients === 2);
const first = await snapshot(local, key);
check(sameIds(a.getState().document, ["base"]) && sameIds(b.getState().document, ["base"]), "both load the shared snapshot");
check(a.getState().revision === first.revision, "A starts at the snapshot's revision");
a.edit(added(a, rect("a1", 10)));
b.edit(added(b, rect("b1", 20)));
for (const [client, name] of [[a, "A"], [b, "B"]] as const) {
  await until(client, `${name} has both strokes`, (s) => settled(s) && sameIds(s.document, ["base", "a1", "b1"]));
}
sameElements(a, b, "(a)");
check(JSON.stringify((await snapshot(local, key)).document.elements) === JSON.stringify(a.getState().document!.elements), "(a) the clients match the server");
// The page's Origin headers above really reach the server: another site's is refused.
const foreign = await fetch(`${local}/api/s/${key.shareId}/commit`, { method: "POST", headers: { Origin: "http://evil.example", Authorization: `Bearer ${key.shareId}.${key.secret}` }, body: "{}" });
check(foreign.status === 403, `a foreign Origin is refused (${foreign.status})`);
const foreignSocket = new WebSocket(`ws://127.0.0.1:${port}/api/s/${key.shareId}/ws`, { headers: { Origin: "http://evil.example" } } as unknown as string[]);
const opened = await within(5_000, "the foreign socket's fate", new Promise<boolean>((resolve) => { foreignSocket.onopen = () => resolve(true); foreignSocket.onclose = () => resolve(false); }));
check(!opened, "a socket from a foreign Origin never opens");
log("(a) two clients converge, Origin checked: ok");

// (b) A desktop commit reaches both.
await desktop("desktop-commit", { id: "d1", seq: 1 });
for (const [client, name] of [[a, "A"], [b, "B"]] as const) {
  await until(client, `${name} sees the desktop's stroke`, (s) => settled(s) && !!find(s.document, "d1"));
}
log("(b) desktop commit reaches both: ok");

// A fresh desktop JS runtime joins the authority that survived its reload.
// Use the real session implementation over serialized native responses/events,
// rather than issuing a synthetic desktop commit directly to the authority.
const desktopFile = { id: 7, relative_path: "courses/TEST/canvases/File7.canvas" };
const desktopListeners = new Set<(event: CanvasAuthorityEvent) => void>();
const snapshotDesktop = () => desktop<Awaited<ReturnType<CanvasAuthorityIO<typeof desktopFile>["snapshot"]>>>("desktop-snapshot");
const unexpected = async (): Promise<never> => { throw new Error("A rejoined shared canvas used the ordinary file path."); };
const desktopSessions = createCanvasSessions({
  read: unexpected, write: unexpected, rename: unexpected, remove: unexpected,
  drafts: new MemoryStorage(),
  authority: {
    existing: snapshotDesktop,
    open: unexpected,
    snapshot: snapshotDesktop,
    commit: (_file, op) => desktop("desktop-op", op),
    resolve: unexpected, rename: unexpected, remove: unexpected, close: unexpected,
    onRemote: (_id, listener) => { desktopListeners.add(listener); return () => { desktopListeners.delete(listener); }; },
  },
});
const desktopSession = desktopSessions.open(desktopFile);
let desktopUpdates = 0;
const stopDesktop = desktopSession.subscribe(() => { desktopUpdates++; });
await desktopSession.load();
check(desktopSession.authority === "live", "a recreated desktop session joins the live native authority");
check(ids(parseCanvasDocument(desktopSession.snapshot().text!)).includes("a1"), "desktop rejoin starts with the device's saved strokes");
await desktop("desktop-events");
b.edit(added(b, rect("rejoined-web", 80)));
await until(b, "the device's new stroke saves after desktop rejoin", (s) => settled(s) && !!find(s.document, "rejoined-web"));
const events = await desktop<CanvasAuthorityEvent[]>("desktop-events");
const beforeUpdates = desktopUpdates;
for (const event of events) for (const listener of desktopListeners) listener(event);
check(desktopUpdates > beforeUpdates && ids(parseCanvasDocument(desktopSession.snapshot().text!)).includes("rejoined-web"), "the mounted desktop receives the device's edit without leaving the canvas");
const desktopDocument = parseCanvasDocument(desktopSession.snapshot().text!);
desktopSession.update(serializeCanvasDocument({ ...desktopDocument, elements: [...desktopDocument.elements, rect("rejoined-desk", 90)] }));
await desktopSession.flush();
for (const [client, name] of [[a, "A"], [b, "B"]] as const) {
  await until(client, `${name} sees the rejoined desktop's edit`, (s) => settled(s) && !!find(s.document, "rejoined-desk"));
}
stopDesktop();
log("(b2) recreated desktop session rejoins; live edits work both ways with no whole-file writes: ok");

// (c) Independent edits merge; the same shape edited on both is a conflict
// for exactly one, whose draft is kept until it takes the shared canvas.
a.edit(moved(a, "a1", 100));
b.edit(moved(b, "b1", 200));
for (const [client, name] of [[a, "A"], [b, "B"]] as const) {
  await until(client, `${name} merges both moves`, (s) => settled(s) && find(s.document, "a1")?.x === 100 && find(s.document, "b1")?.x === 200);
}
sameElements(a, b, "(c) merge");
a.edit(moved(a, "base", 11));
b.edit(moved(b, "base", 22));
await Promise.race([until(a, "a conflict", (s) => s.status === "conflict", 15_000), until(b, "a conflict", (s) => s.status === "conflict", 15_000)]);
const [loser, winner, lost, won] = a.getState().status === "conflict" ? [a, b, 11, 22] : [b, a, 22, 11];
check(loser !== winner && winner.getState().status !== "conflict", "exactly one side conflicts");
await until(winner, "the winner settles", (s) => settled(s) && find(s.document, "base")?.x === won);
const conflicted = loser.getState();
check(conflicted.hasDraft && !!conflicted.lastError, "the conflict keeps a draft and says why");
check(find(conflicted.document, "base")?.x === lost, "the losing side still shows its own edit");
check(find(parseCanvasDocument(loser.draftText()!), "base")?.x === lost, "the draft exports the losing edit");
loser.useSharedCanvas();
await until(loser, "the loser takes the shared canvas", (s) => settled(s) && find(s.document, "base")?.x === won && !s.hasDraft);
sameElements(a, b, "(c) after recovery");
log(`(c) merge, conflict on ${loser === a ? "A" : "B"}, recovery: ok`);

// (d) A 10 MiB PNG goes up as one POST and down to the other tab intact.
const raw = new Uint8Array(10 * 1024 * 1024);
for (let at = 0; at < raw.length; at += 65_536) crypto.getRandomValues(raw.subarray(at, at + 65_536));
raw.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
const dataUrl = `data:image/png;base64,${Buffer.from(raw).toString("base64")}`;
const sha256 = createHash("sha256").update(dataUrl).digest("hex");
a.edit(added(a, { ...rect("img", 300), type: "image", dataUrl, naturalWidth: 1024, naturalHeight: 1024 }));
for (const [client, name] of [[a, "A"], [b, "B"]] as const) {
  await until(client, `${name} has the image`, (s) => settled(s) && find(s.document, "img")?.dataUrl === dataUrl, 90_000);
}
await desktop("expect-image", { id: "img", sha256, length: dataUrl.length });
log(`(d) ${(dataUrl.length / 1048576).toFixed(1)} MiB image round trip: ok`);

// The delete is already queued for the Mac when Undo restores the image locally.
// Its later confirmation must preserve Undo and send the restoration afterward.
const beforeUndo = a.getState().document!;
const undoAt = A.commits.length;
A.holdCommits();
a.edit({ ...beforeUndo, elements: beforeUndo.elements.filter(element => element.id !== "img") });
await within(5_000, "the held image deletion", (async () => { while (A.commits.length === undoAt) await new Promise(resolve => setTimeout(resolve, 5)); })());
a.edit(beforeUndo);
A.release();
for (const [client, name] of [[a, "A"], [b, "B"]] as const) {
  await until(client, `${name} retains the immediately undone image`, s => settled(s) && find(s.document, "img")?.dataUrl === dataUrl, 90_000);
}
check(A.commits.length === undoAt + 2, "the image deletion and its undo are each committed once");
await desktop("expect-image", { id: "img", sha256, length: dataUrl.length });
log("(d2) quick image undo survives delayed native confirmation: ok");

// (e) The first answer to a commit is lost after the Mac applied it: the
// retry is the same op, the Mac replays its ack, and the stroke exists once.
const before = A.commits.length;
A.dropNextAnswer = true;
a.edit(added(a, rect("e1", 400)));
await until(a, "A retries the lost commit", (s) => settled(s) && !!find(s.document, "e1") && A.commits.length >= before + 2, 15_000);
const [lostCommit, retried] = A.commits.slice(before);
check(lostCommit.dropped && lostCommit.status === 200, "the first answer was a 200 that never reached the page");
check(retried.body.clientId === lostCommit.body.clientId && retried.body.seq === lostCommit.body.seq && retried.body.opId === lostCommit.body.opId, "the retry is the same op");
check(retried.status === 200 && retried.answer?.replayed === true && retried.answer?.transaction === null, `the Mac replays its ack (${JSON.stringify(retried.answer)})`);
check(retried.answer?.revision === lostCommit.answer?.revision, "the replay names the original revision");
check(A.commits.length === before + 2, "no third commit");
check((await snapshot(local, key)).document.elements.filter((el) => el.id === "e1").length === 1, "the server has e1 exactly once");
await until(b, "B sees e1 once", (s) => settled(s) && ids(s.document).filter((id) => id === "e1").length === 1);
log("(e) lost answer replayed once: ok");

// (f) The socket dies; an edit made while it is down waits, then lands.
const socketsBefore = A.sockets.length;
const commitsBefore = A.commits.length;
A.sockets.at(-1)!.close();
await until(a, "A notices the socket is gone", (s) => s.status === "reconnecting" || s.status === "offline-unsaved");
a.edit(added(a, rect("f1", 500)));
check(a.getState().status === "offline-unsaved" && a.getState().hasDraft, `an edit while down is unsaved, not lost (${describe(a.getState())})`);
check(A.commits.length === commitsBefore, "nothing is sent while the connection is down");
await until(a, "A reconnects and saves", (s) => settled(s) && !!find(s.document, "f1"), 15_000);
await until(b, "B sees f1", (s) => settled(s) && !!find(s.document, "f1"));
check(A.sockets.length === socketsBefore + 1, "one new socket");
check(A.commits.slice(commitsBefore).filter((c) => c.body.transaction.edits.some((edit) => edit.id === "f1")).length === 1, "f1 committed once");
sameElements(a, b, "(f)");
log(`(f) socket loss: ${A.statuses.slice(-4).join(" → ")}: ok`);

// The text pings kept every socket alive: B never reconnected, A only when made to.
check(B.sockets.length === 1 && A.sockets.length === 3, `sockets opened: A ${A.sockets.length}, B ${B.sockets.length}`);
check(A.pongs > 0 && B.pongs > 0, "both answered the server's text pings");

// (g) The Mac stops sharing while A has an edit on its way: both end, and
// both keep a drawing they can export.
A.holdCommits();
const heldAt = A.commits.length;
a.edit(added(a, rect("g1", 600)));
await within(5_000, "A's held commit", (async () => { while (A.commits.length === heldAt) await new Promise((resolve) => setTimeout(resolve, 5)); })());
const heldOp = A.commits.at(-1)!.body;
await desktop("stop-share");
for (const [client, name] of [[a, "A"], [b, "B"]] as const) {
  await until(client, `${name} ends`, (s) => s.status === "ended");
}
A.release();
await within(5_000, "the held commit's answer", (async () => { while (A.commits.at(-1)!.status === undefined) await new Promise((resolve) => setTimeout(resolve, 5)); })());
check(A.commits.at(-1)!.status === 410 && A.commits.at(-1)!.answer?.code === "session-ended", `a commit after Stop is refused (${A.commits.at(-1)!.status} ${JSON.stringify(A.commits.at(-1)!.answer)})`);
check(a.getState().status === "ended" && a.getState().hasDraft, "A ended with an unsaved draft");
for (const [client, name] of [[a, "A"], [b, "B"]] as const) {
  const message = client.getState().lastError ?? "";
  check(message.length > 20 && !/^(stopped|ended|shutdown)$/.test(message), `${name} explains the end in words, not a code: "${message}"`);
}
const draftA = parseCanvasDocument(a.draftText()!);
check(!!find(draftA, "g1") && find(draftA, "img")?.dataUrl === dataUrl, "A's draft exports its unsaved stroke and the image");
const draftB = parseCanvasDocument(b.draftText()!);
check(!find(draftB, "g1") && !!find(draftB, "f1"), "B's draft is the last shared drawing");
// The complete image-bearing drawing remains recoverable after the share ends.
check(A.storage.getItem(draftKey(key.shareId))?.includes("\"g1\""), "A's unsaved edit is stored for a reload");
check(!a.getState().storageWarning, "A keeps the complete drawing without a size warning");
log("(g) Stop: both ended, drafts kept: ok");

// (h) Nothing from the ended epoch reaches the new one.
const next = await desktop<{ key: string }>("restart-share");
const newKey = parseShareFragment(next.key)!;
check(!!newKey && newKey.shareId !== key.shareId, "a restarted share has a new link");
const reloaded = new Device("A reloaded", local, A.storage);
const r = reloaded.client(key);
r.start();
await until(r, "the old link ends on reload", (s) => s.status === "ended");
check(reloaded.commits.length === 0 && !!find(r.getState().document, "g1"), "a reload of the old link sends nothing and replays nothing");
const sent = A.commits.length;
a.retry();
await new Promise((resolve) => setTimeout(resolve, 0));
check(A.commits.length === sent && a.getState().status === "ended", "retry after the end does nothing");
for (const credentials of [key, newKey]) {
  const answer = await fetch(`${local}/api/s/${credentials.shareId}/commit`, {
    method: "POST",
    headers: { Authorization: `Bearer ${credentials.shareId}.${credentials.secret}`, "Content-Type": "application/json", Origin: local },
    body: JSON.stringify(heldOp),
  });
  const body = await answer.json() as { code?: string };
  check(answer.status === 410 && body.code === "session-ended", `the old op is refused by ${credentials === key ? "the old" : "the new"} share (${answer.status} ${body.code})`);
}
const fresh = await snapshot(local, newKey);
check(fresh.epoch !== first.epoch && !find(fresh.document, "g1"), "the new epoch has no trace of the old draft");
const C = new Device("C", local, A.storage);
const c = C.client(newKey);
c.start();
await until(c, "C joins the new share", (s) => settled(s) && !find(s.document, "g1") && !s.hasDraft && s.recovery === null);
c.edit(added(c, rect("h1", 700)));
await until(c, "C saves in the new epoch", (s) => settled(s) && !!find(s.document, "h1"));
check(C.commits.length === 1 && C.commits[0].body.epoch === fresh.epoch && C.commits[0].body.clientId !== heldOp.clientId, "C commits once, in the new epoch, under a new client id");
log("(h) no replay into a new epoch: ok");

for (const client of [a, b, r, c]) client.dispose();
await desktop("done");
stdin.close();
log("all scenarios passed");
process.exit(0);
