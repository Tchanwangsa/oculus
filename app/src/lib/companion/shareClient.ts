import { applyCanvasTransaction, diffCanvasContent, hasCanvasChanges, type CanvasTransaction } from "@/lib/canvas/collaboration";
import { clientPendingTransactions, containsClientTransaction, reconcileClientDocument } from "@/lib/canvas/clientReconciliation";
import { parseCanvasDocument, serializeCanvasDocument, type CanvasDocument } from "@/lib/canvas/model";
import { ShareApi, ShareHttpError } from "./api";
import { clearRecovery, readDraft, readRecovery, writeDraft, writeRecovery, type ShareDraftRecord, type ShareRecovery } from "./drafts";
import { backoffDelay, type ShareClientEnv, type ShareLifecycleEvent, type WebSocketLike } from "./env";
import { redactSecret, type ShareCredentials } from "./link";
import { parseFrame, type ServerFrame, type ShareAck, type ShareOp, type ShareSnapshot } from "./protocol";

/**
 * A paired browser's side of a shared canvas (wire protocol v1, docs/device-sharing.md).
 *
 * The local drawing is the shared canvas (`canonical`, at `revision`) with this
 * device's unconfirmed edits on top. At most one commit is in flight: its
 * transaction is the difference between the last confirmed canonical drawing and
 * the local one, sent over HTTP with a fixed (clientId, seq, opId), so a retry
 * after a lost answer is the same op and the server replays its ack. Remote
 * changes advance `canonical` in revision order and the local edits are rebased
 * onto them. The socket only announces revisions, presence and lifecycle.
 */

export type ShareStatus = "connecting" | "connected" | "saving" | "reconnecting" | "offline-unsaved" | "conflict" | "paused" | "ended";

export interface ShareClientState {
  status: ShareStatus;
  /** The drawing as this device sees it; null until the first snapshot. */
  document: CanvasDocument | null;
  /** Unconfirmed ops: the one in flight plus the edits waiting behind it (0–2). */
  pendingOps: number;
  /** Edits the Mac has not confirmed exist, so "Download draft" has something to save. */
  hasDraft: boolean;
  clients: number;
  revision: number;
  lastError: string | null;
  /** The draft is too large for, or refused by, this browser's storage: a reload would lose it. */
  storageWarning: string | null;
  /** A draft from an earlier sharing session of this link, offered for download only. */
  recovery: { savedAt: number } | null;
  /** When the next automatic attempt runs (clock time), while one is scheduled. */
  retryAt: number | null;
}

export interface ShareClientOptions {
  /** No frame for this long means the socket is dead (the server pings every 20 s). */
  deadManMs?: number;
  /** Debounce for writing the draft after an edit. */
  persistDelayMs?: number;
}

const KEPT = "Your drawing here is kept: download it to keep your latest changes.";
const ENDED = `Sharing was stopped on the Mac. ${KEPT}`;
/** `session-ended` carries a reason code, never text to show. */
const ENDED_BECAUSE: Record<string, string> = {
  stopped: ENDED,
  ended: `The canvas was closed on the Mac. ${KEPT}`,
  shutdown: `Oculus quit on the Mac. ${KEPT}`,
};
const NEW_SESSION = "The Mac started a new sharing session. Changes from before are not added to it.";
const LOST = "Lost the connection to the Mac.";
const SILENT = "No word from the Mac for a while.";
const STORAGE_FAILED = "This browser is not keeping a copy of unsaved changes. Keep this tab open until they are saved.";

/** Bytes of a commit body, for its timeout: 20 s plus 10 s per MB, so large images on a slow network still finish. */
const commitTimeout = (op: ShareOp) => 20_000 + JSON.stringify(op.transaction).length / 100;

const ENDING_CODES = new Set(["session-ended", "gone", "unauthorized", "not-found"]);

export class ShareClient {
  private readonly api: ShareApi;
  private readonly deadManMs: number;
  private readonly persistDelayMs: number;

  private canonical: CanvasDocument | null = null;
  private local: CanvasDocument | null = null;
  private epoch: string | null = null;
  private revision = 0;
  /** The highest revision announced; catching up continues until it is reached. */
  private announced = 0;
  /** Pull once even without an announcement: after a conflict the server is newer than we know. */
  private forcePull = false;
  private clientId: string;
  private nextSeq = 1;
  private inflight: ShareOp | null = null;
  /** Its revision reached us before its HTTP reply; subsequent changes must not replay it. */
  private inflightSeen = false;
  /** Local edits made since the op in flight was cut. */
  private dirty = false;
  private conflict: string | null = null;
  private paused = false;
  private ended = false;

  private socket: WebSocketLike | null = null;
  private welcomed = false;
  private clients = 0;
  private lastFrame = 0;
  private deadMan: unknown = null;
  /** Waiting for the next attempt after a failure; commits hold until then. */
  private down = false;
  private attempt = 0;
  private retryTimer: unknown = null;
  private retryAt: number | null = null;
  private everLoaded = false;

  private loading: Promise<void> | null = null;
  private catching: Promise<void> | null = null;
  private resyncing: Promise<void> | null = null;
  private pumping = false;
  private pumpAgain = false;
  private persistTimer: unknown = null;
  private persistence: Promise<void> = Promise.resolve();
  private persistVersion = 0;
  private restored: ShareDraftRecord | null = null;
  private restoring: Promise<void> | null = null;

  private lastError: string | null = null;
  private storageWarning: string | null = null;
  private recovery: ShareRecovery | null = null;
  private started = false;
  private disposed = false;
  private stopLifecycle: (() => void) | null = null;
  private readonly listeners = new Set<() => void>();
  private state: ShareClientState;

  constructor(private readonly credentials: ShareCredentials, private readonly env: ShareClientEnv, options: ShareClientOptions = {}) {
    this.api = new ShareApi(env.origin, credentials.shareId, credentials.secret, env.fetch);
    this.deadManMs = options.deadManMs ?? 45_000;
    this.persistDelayMs = options.persistDelayMs ?? 400;
    this.clientId = env.newId();
    this.state = this.compute();
  }

  getState = (): ShareClientState => this.state;

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => { this.listeners.delete(listener); };
  };

  start(): void {
    if (this.started || this.disposed) return;
    this.started = true;
    this.stopLifecycle = this.env.lifecycle((event) => this.onLifecycle(event));
    void this.restore().then(() => { if (!this.disposed) this.connect(); });
  }

  private restore(): Promise<void> {
    return this.restoring ??= Promise.all([
      readDraft(this.env.storage, this.credentials.shareId),
      readRecovery(this.env.storage, this.credentials.shareId),
    ]).then(([record, recovery]) => { this.restored = record; this.recovery = recovery; });
  }

  dispose(): void {
    if (this.disposed) return;
    this.persistNow();
    this.disposed = true;
    this.stopLifecycle?.();
    this.closeSocket();
    this.clearRetry();
    if (this.persistTimer !== null) this.env.clock.clearTimeout(this.persistTimer);
    this.listeners.clear();
  }

  /** The pane's edit: a whole drawing, whose content difference becomes the next commit. */
  edit(document: CanvasDocument): void {
    if (!this.local || this.ended || this.conflict || document === this.local) return;
    this.local = document;
    if (this.inflight) this.dirty = true;
    this.publish();
    this.schedulePersist();
    void this.pump();
  }

  /** Conflict: drop this device's unsaved edits and show the shared canvas. */
  useSharedCanvas(): void {
    if (!this.canonical || this.ended) return;
    if (this.inflight) {
      // Its fate is unknown, so its seq is never reused for another op.
      this.nextSeq = Math.max(this.nextSeq, this.inflight.seq + 1);
      this.inflight = null;
      this.inflightSeen = false;
    }
    this.conflict = null;
    this.dirty = false;
    this.lastError = null;
    this.local = { ...this.canonical, viewport: this.local?.viewport ?? this.canonical.viewport };
    this.persistNow();
    this.publish();
    void this.pump();
  }

  /** The local drawing as a `.canvas` file: the shared canvas with this device's unsaved edits. */
  draftText(): string | null {
    return this.local ? serializeCanvasDocument(this.local) : null;
  }

  recoveryText(): string | null { return this.recovery?.draft ?? null; }

  dismissRecovery(): void {
    this.recovery = null;
    this.persistence = this.persistence.then(() => clearRecovery(this.env.storage, this.credentials.shareId));
    this.publish();
  }

  /** Try again now: a paused save, or a connection waiting for its next attempt. */
  retry(): void {
    if (this.ended || this.disposed) return;
    this.paused = false;
    this.lastError = null;
    void this.restore().then(() => { if (!this.disposed) this.connect(); });
  }

  private compute(): ShareClientState {
    const unconfirmed = this.unconfirmed();
    const pendingOps = this.inflight ? 1 + (this.dirty ? 1 : 0) : unconfirmed ? 1 : 0;
    let status: ShareStatus;
    if (this.ended) status = "ended";
    else if (this.conflict) status = "conflict";
    else if (this.paused) status = "paused";
    else if (!this.canonical) status = this.everLoaded || this.attempt > 0 ? "reconnecting" : "connecting";
    else if (this.welcomed && !this.down) status = pendingOps ? "saving" : "connected";
    else status = unconfirmed ? "offline-unsaved" : "reconnecting";
    return {
      status,
      document: this.local,
      pendingOps,
      hasDraft: unconfirmed,
      clients: this.clients,
      revision: this.revision,
      lastError: this.conflict ?? this.lastError,
      storageWarning: unconfirmed ? this.storageWarning : null,
      recovery: this.recovery ? { savedAt: this.recovery.savedAt } : null,
      retryAt: this.retryAt,
    };
  }

  private publish(): void {
    if (this.disposed) return;
    this.state = this.compute();
    for (const listener of this.listeners) listener();
  }

  private unconfirmed(): boolean {
    if (!this.local) return false;
    if (this.inflight || this.conflict) return true;
    if (!this.canonical) return true;
    return this.local !== this.canonical && hasCanvasChanges(diffCanvasContent(this.canonical, this.local));
  }

  private setError(message: string | null): void {
    this.lastError = message === null ? null : redactSecret(message, this.credentials);
  }

  private setConflict(message: string): void {
    this.conflict = redactSecret(message, this.credentials);
  }

  private onLifecycle(event: ShareLifecycleEvent): void {
    if (event === "pagehide") { this.persistNow(); return; }
    if (this.ended || this.disposed) return;
    const silent = this.socket !== null && this.env.clock.now() - this.lastFrame >= this.deadManMs;
    if (silent) this.closeSocket();
    if (this.down || !this.socket || !this.canonical || silent) this.connect();
  }

  /** One attempt at whatever is missing: the snapshot, the socket, the commit. */
  private connect(): void {
    if (this.disposed || this.ended) return;
    this.clearRetry();
    this.down = false;
    this.publish();
    if (!this.canonical) { void this.load(); return; }
    this.openSocket();
    void this.pump();
  }

  private clearRetry(): void {
    if (this.retryTimer !== null) this.env.clock.clearTimeout(this.retryTimer);
    this.retryTimer = null;
    this.retryAt = null;
  }

  /** Something failed on the way to the Mac: wait, then try everything again. */
  private trouble(message: string): void {
    if (this.disposed || this.ended) return;
    this.setError(message);
    this.down = true;
    this.closeSocket();
    if (this.retryTimer === null) {
      const delay = backoffDelay(this.attempt++, this.env.random);
      this.retryAt = this.env.clock.now() + delay;
      this.retryTimer = this.env.clock.setTimeout(() => { this.retryTimer = null; this.retryAt = null; this.connect(); }, delay);
    }
    this.publish();
  }

  private end(message: string): void {
    if (this.ended) return;
    this.ended = true;
    this.setError(message);
    this.closeSocket();
    this.clearRetry();
    this.persistNow();
    this.publish();
  }

  /** A failed request: an ending code ends the share, anything else is retried later. */
  private failed(error: unknown): void {
    if (error instanceof ShareHttpError && ENDING_CODES.has(error.code)) { this.end(endMessage(error)); return; }
    this.trouble(error instanceof Error ? error.message : LOST);
  }

  private load(): Promise<void> {
    this.loading ??= (async () => {
      let snapshot: ShareSnapshot;
      try { snapshot = await this.api.snapshot(); }
      catch (error) {
        if (error instanceof ShareHttpError && ENDING_CODES.has(error.code)) this.restoreEndedDraft();
        this.failed(error);
        return;
      }
      if (this.disposed || this.ended) return;
      let document: CanvasDocument;
      try { document = parseCanvasDocument(snapshot.text); }
      catch (error) { this.trouble(`The Mac sent a canvas this page cannot read: ${(error as Error).message}`); return; }
      this.adopt(snapshot, document);
      this.attempt = 0;
      this.setError(null);
      this.openSocket();
      void this.pump();
    })().finally(() => { this.loading = null; });
    return this.loading;
  }

  /** The first snapshot of this page: resume a draft from this epoch, or keep one from another for download. */
  private adopt(snapshot: ShareSnapshot, document: CanvasDocument): void {
    this.canonical = document;
    this.local = document;
    this.epoch = snapshot.epoch;
    this.revision = this.announced = snapshot.revision;
    this.everLoaded = true;
    const record = this.restored;
    if (record && record.epoch === snapshot.epoch) {
      this.clientId = record.clientId;
      this.nextSeq = record.seqFloor;
      this.inflight = record.inflight;
      if (record.draft && (record.conflict || (!record.pending && !record.inflight))) {
        this.setConflict(record.conflict ?? "This device has a kept draft. Download it or use the shared canvas.");
        try { this.local = parseCanvasDocument(record.draft); } catch { /* keep the shared canvas */ }
      } else if (record.pending || (record.inflight && record.afterInflight !== undefined)) {
        try {
          const seen = record.inflightRevision !== undefined && record.inflightRevision <= snapshot.revision;
          const pending: CanvasTransaction[] = [];
          if (record.inflight && record.afterInflight !== undefined) {
            if (!seen) pending.push(record.inflight.transaction);
            if (record.afterInflight) pending.push(record.afterInflight);
          } else if (record.pending) pending.push(record.pending);
          this.local = reconcileClientDocument(document, pending, document);
          this.inflightSeen = !!record.inflight && (seen || containsClientTransaction(document, record.inflight.transaction));
          this.dirty = !!record.afterInflight;
        }
        catch (error) {
          this.setConflict((error as Error).message);
          if (record.draft) try { this.local = parseCanvasDocument(record.draft); } catch { /* keep the shared canvas */ }
        }
      }
    } else if (record?.draft) {
      this.recovery = { epoch: record.epoch, draft: record.draft, savedAt: record.savedAt };
      const recovery = this.recovery;
      this.persistence = this.persistence.then(async () => {
        if (!await writeRecovery(this.env.storage, this.credentials.shareId, recovery)) {
          this.storageWarning = STORAGE_FAILED;
          this.publish();
        }
      });
    }
    this.persistNow();
    this.publish();
  }

  /** The share is gone before this page loaded it: a stored draft is still downloadable. */
  private restoreEndedDraft(): void {
    const record = this.restored;
    if (!record?.draft || this.local) return;
    try { this.local = parseCanvasDocument(record.draft); this.epoch = record.epoch; } catch { /* nothing to offer */ }
  }

  private openSocket(): void {
    if (this.socket || this.disposed || this.ended) return;
    const url = `${this.env.origin.replace(/^http/, "ws")}/api/s/${this.credentials.shareId}/ws`;
    let socket: WebSocketLike;
    try { socket = this.env.openSocket(url); }
    catch { this.trouble(LOST); return; }
    this.socket = socket;
    this.welcomed = false;
    this.lastFrame = this.env.clock.now();
    this.armDeadMan();
    socket.onopen = () => {
      if (this.socket !== socket) return;
      this.lastFrame = this.env.clock.now();
      this.armDeadMan();
      socket.send(JSON.stringify({ type: "hello", clientId: this.clientId, secret: this.credentials.secret, epoch: this.epoch ?? undefined, revision: this.revision }));
    };
    socket.onmessage = (event) => {
      if (this.socket !== socket) return;
      this.lastFrame = this.env.clock.now();
      this.armDeadMan();
      const frame = parseFrame(event.data);
      if (frame) this.onFrame(socket, frame);
    };
    socket.onclose = () => {
      if (this.socket !== socket) return;
      this.socket = null;
      this.welcomed = false;
      this.clearDeadMan();
      this.trouble(LOST);
    };
    socket.onerror = () => {};
  }

  private closeSocket(): void {
    const socket = this.socket;
    this.socket = null;
    this.welcomed = false;
    this.clearDeadMan();
    if (socket) try { socket.close(1000); } catch { /* already closed */ }
  }

  private armDeadMan(): void {
    this.clearDeadMan();
    this.deadMan = this.env.clock.setTimeout(() => {
      this.deadMan = null;
      if (!this.socket) return;
      this.closeSocket();
      this.trouble(SILENT);
    }, this.deadManMs);
  }

  private clearDeadMan(): void {
    if (this.deadMan !== null) this.env.clock.clearTimeout(this.deadMan);
    this.deadMan = null;
  }

  private onFrame(socket: WebSocketLike, frame: ServerFrame): void {
    switch (frame.type) {
      case "welcome":
        if (this.epoch && frame.epoch !== this.epoch) { this.end(NEW_SESSION); return; }
        this.welcomed = true;
        this.attempt = 0;
        this.clients = frame.clients;
        if (this.lastError === LOST || this.lastError === SILENT) this.setError(null);
        this.announce(frame.revision);
        this.publish();
        void this.pump();
        return;
      case "revision":
        if (frame.epoch !== this.epoch) { this.end(NEW_SESSION); return; }
        this.announce(frame.revision);
        return;
      case "change":
        if (frame.epoch !== this.epoch) { this.end(NEW_SESSION); return; }
        if (frame.revision === this.revision + 1 && !this.catching && !this.resyncing) {
          if (!this.advance(frame.revision, frame.transaction, frame.clientId === `web:${this.clientId}`)) void this.resync();
          this.announced = Math.max(this.announced, frame.revision);
        } else this.announce(frame.revision);
        return;
      case "presence":
        this.clients = frame.clients;
        this.publish();
        return;
      case "state":
        if (frame.phase === "ended") { this.end(ENDED); return; }
        if (frame.phase === "paused") { this.paused = true; this.publish(); return; }
        if (this.paused) { this.paused = false; this.setError(null); this.publish(); void this.pump(); }
        return;
      case "session-ended":
        this.end(ENDED_BECAUSE[frame.reason] ?? ENDED);
        return;
      case "ping":
        try { socket.send(JSON.stringify({ type: "pong" })); } catch { /* the close handler reconnects */ }
        return;
    }
  }

  private announce(revision: number): void {
    this.announced = Math.max(this.announced, revision);
    if (revision > this.revision) void this.catchUp();
  }

  /** Applies the next revision to the shared canvas and rebases the local edits; false when it cannot apply. */
  private advance(revision: number, transaction: CanvasTransaction, own = false): boolean {
    const before = this.canonical!;
    try { this.canonical = applyCanvasTransaction(before, transaction); }
    catch { return false; }
    this.revision = revision;
    this.rebase(before, own);
    return true;
  }

  /** Keeps this device's edits (the difference from `before`) on top of the new shared canvas. */
  private rebase(before: CanvasDocument, own = false): void {
    if (!this.local || !this.canonical) return;
    if (this.conflict) { this.publish(); return; }
    try {
      const inflight = !this.inflightSeen ? this.inflight?.transaction : undefined;
      const pending = clientPendingTransactions(before, this.local, inflight);
      const next = reconcileClientDocument(this.canonical, own && inflight ? pending.slice(1) : pending, this.local);
      if (own && this.inflight) this.inflightSeen = true;
      if (next !== this.local) { this.local = next; this.schedulePersist(); }
    } catch (error) {
      this.setConflict((error as Error).message);
      this.persistNow();
    }
    this.publish();
  }

  /** Pulls the changes after `revision`; a log that no longer reaches back means a fresh snapshot. */
  private catchUp(force = false): Promise<void> {
    if (force) this.forcePull = true;
    this.catching ??= (async () => {
      while (!this.ended && !this.disposed && !this.down && (this.forcePull || this.announced > this.revision)) {
        if (this.resyncing) { await this.resyncing; continue; }
        this.forcePull = false;
        const from = this.revision;
        let changes;
        try { changes = await this.api.changes(from); }
        catch (error) {
          if (error instanceof ShareHttpError && error.code === "gone") { await this.resync(); continue; }
          this.failed(error);
          return;
        }
        if (this.ended || this.disposed) return;
        let progressed = false;
        for (const change of changes) {
          if (change.epoch !== this.epoch) { this.end(NEW_SESSION); return; }
          if (change.revision <= this.revision) continue;
          if (change.revision !== this.revision + 1 || !this.advance(change.revision, change.transaction, change.clientId === `web:${this.clientId}`)) { await this.resync(); progressed = true; break; }
          progressed = true;
          this.announced = Math.max(this.announced, change.revision);
        }
        // The log is behind its announcement: the snapshot settles it.
        if (!progressed && this.announced > this.revision) {
          await this.resync();
          if (!this.down) this.announced = this.revision;
        }
      }
    })().finally(() => { this.catching = null; });
    return this.catching;
  }

  /** Replaces the shared canvas with a fresh snapshot, rebasing the local edits onto it. */
  private resync(): Promise<void> {
    this.resyncing ??= (async () => {
      let snapshot: ShareSnapshot;
      try { snapshot = await this.api.snapshot(); }
      catch (error) { this.failed(error); return; }
      if (this.ended || this.disposed) return;
      if (snapshot.epoch !== this.epoch) { this.end(NEW_SESSION); return; }
      if (snapshot.revision < this.revision) return;
      let document: CanvasDocument;
      try { document = parseCanvasDocument(snapshot.text); }
      catch (error) { this.trouble(`The Mac sent a canvas this page cannot read: ${(error as Error).message}`); return; }
      const before = this.canonical!;
      this.canonical = document;
      this.revision = snapshot.revision;
      this.announced = Math.max(this.announced, snapshot.revision);
      this.rebase(before);
      if (this.inflight && !this.inflightSeen && !this.conflict) this.inflightSeen = containsClientTransaction(document, this.inflight.transaction);
    })().finally(() => { this.resyncing = null; });
    return this.resyncing;
  }

  private canCommit(): boolean {
    return !this.disposed && !this.ended && !this.conflict && !this.paused && !this.down && !!this.canonical && !!this.local && !!this.epoch;
  }

  /** Sends the local edits one op at a time until the Mac has them all. */
  private async pump(): Promise<void> {
    if (this.pumping) { this.pumpAgain = true; return; }
    this.pumping = true;
    try {
      while (this.canCommit()) {
        if (this.catching) await this.catching;
        if (this.resyncing) await this.resyncing;
        if (!this.canCommit()) break;
        let op = this.inflight;
        if (!op) {
          const transaction = diffCanvasContent(this.canonical!, this.local!);
          if (!hasCanvasChanges(transaction)) { this.dirty = false; break; }
          op = { clientId: this.clientId, seq: this.nextSeq, opId: this.env.newId(), epoch: this.epoch!, transaction };
          this.inflight = op;
          this.inflightSeen = false;
          this.dirty = false;
          // Stored before it is sent, so a reload resends this op rather than reusing its seq.
          await this.persistNow();
          if (!this.canCommit() || this.inflight !== op) break;
        }
        this.publish();
        let ack: ShareAck;
        try { ack = await this.send(op); }
        catch (error) {
          if (this.inflight !== op) break;
          if (await this.refused(error)) continue;
          break;
        }
        if (this.inflight !== op) break;
        this.nextSeq = op.seq + 1;
        if (this.lastError && !this.down) this.setError(null);
        await this.acknowledge(ack);
        if (this.inflight !== op) break;
        // Our own change is not in the shared canvas yet: wait for it rather than send it twice.
        if (this.revision < ack.revision) break;
        this.inflight = null;
        this.inflightSeen = false;
        this.persistNow();
      }
    } finally {
      this.pumping = false;
      this.publish();
      if (this.pumpAgain) { this.pumpAgain = false; void this.pump(); }
    }
  }

  private send(op: ShareOp): Promise<ShareAck> {
    const controller = new AbortController();
    const timer = this.env.clock.setTimeout(() => controller.abort(), commitTimeout(op));
    return this.api.commit(op, controller.signal).finally(() => this.env.clock.clearTimeout(timer));
  }

  /** Decides what a refused or unanswered commit means; true sends again at once. */
  private async refused(error: unknown): Promise<boolean> {
    if (!(error instanceof ShareHttpError) || error.code === "server" || error.code === "bad-response") {
      // No answer, or one that may hide an applied op: keep it and resend it unchanged.
      this.trouble(error instanceof Error ? error.message : LOST);
      return false;
    }
    // A refusal: nothing was applied and the op's seq is free again.
    this.inflight = null;
    this.inflightSeen = false;
    this.dirty = false;
    switch (error.code) {
      case "stale-op":
        this.clientId = this.env.newId();
        this.nextSeq = 1;
        this.persistNow();
        await this.resync();
        return this.canCommit();
      case "external-edit":
        this.paused = true;
        this.setError(error.message);
        this.publish();
        return false;
      case "io":
        this.trouble(error.message);
        return false;
      default:
        if (ENDING_CODES.has(error.code)) { this.end(endMessage(error)); return false; }
        // conflict-*, invalid, too-large: this draft cannot be shared as it is.
        this.setConflict(error.message);
        this.persistNow();
        this.publish();
        // The server holds something newer than this page has seen.
        if (error.code.startsWith("conflict-")) void this.catchUp(true);
        return false;
    }
  }

  private async acknowledge(ack: ShareAck): Promise<void> {
    if (ack.epoch !== this.epoch) { this.end(NEW_SESSION); return; }
    this.announced = Math.max(this.announced, ack.revision);
    if (ack.revision <= this.revision) return;
    if (ack.transaction && ack.revision === this.revision + 1 && !this.catching && !this.resyncing && this.advance(ack.revision, ack.transaction, true)) return;
    await this.catchUp();
  }

  private schedulePersist(): void {
    if (this.persistTimer !== null || this.disposed) return;
    this.persistTimer = this.env.clock.setTimeout(() => { this.persistTimer = null; this.persistNow(); }, this.persistDelayMs);
  }

  private persistNow(): Promise<void> {
    if (this.persistTimer !== null) { this.env.clock.clearTimeout(this.persistTimer); this.persistTimer = null; }
    if (this.disposed || !this.epoch) return this.persistence;
    const unconfirmed = this.unconfirmed();
    let pending: CanvasTransaction | null = null;
    let afterInflight: CanvasTransaction | null | undefined;
    if (unconfirmed && this.canonical && this.local && !this.conflict) {
      const transaction = diffCanvasContent(this.canonical, this.local);
      pending = hasCanvasChanges(transaction) ? transaction : null;
      if (this.inflight) {
        // Keep the undo separately even when the aggregate pending diff is empty.
        const base = this.inflightSeen ? this.canonical : applyCanvasTransaction(this.canonical, this.inflight.transaction);
        const after = diffCanvasContent(base, this.local);
        afterInflight = hasCanvasChanges(after) ? after : null;
      }
    }
    let draft: string | null = null;
    if (unconfirmed && this.local) {
      try { draft = serializeCanvasDocument(this.local); } catch { draft = null; }
    }
    const record: ShareDraftRecord = {
      v: 1, epoch: this.epoch, clientId: this.clientId,
      seqFloor: this.inflight ? Math.max(this.nextSeq, this.inflight.seq + 1) : this.nextSeq,
      inflight: this.inflight, pending, afterInflight,
      ...(this.inflight && this.inflightSeen ? { inflightRevision: this.revision } : {}),
      ...(this.conflict ? { conflict: this.conflict } : {}),
      draft, savedAt: this.env.clock.now(),
    };
    const version = ++this.persistVersion;
    this.persistence = this.persistence.then(async () => {
      const kept = await writeDraft(this.env.storage, this.credentials.shareId, record);
      if (version !== this.persistVersion) return;
      const warning = !this.unconfirmed() || kept ? null : STORAGE_FAILED;
      if (warning !== this.storageWarning) { this.storageWarning = warning; this.publish(); }
    });
    return this.persistence;
  }
}

function endMessage(error: ShareHttpError): string {
  return error.code === "unauthorized" ? "This share link is no longer valid. Ask for a new link on the Mac." : ENDED;
}
