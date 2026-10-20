/** One serialized write queue per file, shared by every mounted pane. While the
 * file is shared the same queue commits content transactions to the canvas
 * authority instead of writing the file, and merges everyone else's changes
 * under this desktop's unsent edits (docs/drawing-canvas.md, "One save session
 * owns every pane editing a file"). */
import { clientPendingTransactions, containsClientTransaction, reconcileClientDocument } from "./clientReconciliation";
import { applyCanvasTransaction, diffCanvasContent, hasCanvasChanges, sameValue, type CanvasTransaction } from "./collaboration";
import { secureId } from "./ids";
import { parseCanvasDocument, serializeCanvasDocument, type CanvasDocument, type CanvasViewport } from "./model";
import {
  authorityCode, errorMessage,
  type CanvasAuthorityAck, type CanvasAuthorityEvent, type CanvasAuthorityIO, type CanvasAuthorityOp,
  type CanvasAuthorityPhase, type CanvasAuthoritySnapshot, type CanvasExternalChoice,
} from "./authority";

export interface CanvasFileRef { id: number; relative_path: string }
export interface CanvasSnapshot {
  text: string | null;
  status: "loading" | "saved" | "unsaved" | "saving" | "error" | "deleted";
  error?: string;
  recoveryError?: string;
  recovery?: { text: string; valid: boolean };
  authority: CanvasAuthorityPhase;
  /** A collaborator's change clashed with this draft; it is kept, unsent, until resolved. */
  conflict?: string;
}
export interface CanvasSessionIO<F extends CanvasFileRef> {
  read(path: string): Promise<string>;
  write(file: F, text: string): Promise<void>;
  rename(file: F, title: string): Promise<F>;
  remove(file: F): Promise<void>;
  drafts: {
    getItem(key: string): string | null | Promise<string | null>;
    setItem(key: string, value: string): void | Promise<void>;
    removeItem(key: string): void | Promise<void>;
  };
  delay?: number;
  validate?(text: string): void;
  /** Absent where sharing is unavailable. */
  authority?: CanvasAuthorityIO<F>;
}
export interface CanvasSession<F extends CanvasFileRef> {
  file: F;
  readonly authority: CanvasAuthorityPhase;
  snapshot(): CanvasSnapshot;
  subscribe(listener: () => void): () => void;
  load(): Promise<void>;
  update(text: string): void;
  flush(): Promise<void>;
  rename(title: string): Promise<void>;
  remove(): Promise<void>;
  recoverDraft(): void;
  discardDraft(): void;
  release(): void;
  /** Flushes to disk, then hands the file to the authority; later saves are commits. */
  attachAuthority(): Promise<void>;
  /** Commits what it can, ends the epoch and saves through the file again. */
  detachAuthority(): Promise<void>;
  /** Drops this desktop's unsent change and shows the shared canvas. */
  useSharedCanvas(): Promise<void>;
  /** Ends a pause after an outside edit: adopt the file on disk or overwrite it. */
  resolveExternal(choice: CanvasExternalChoice): Promise<void>;
}

const PAUSED = "The canvas file changed outside Oculus. Choose which version to keep.";
const ENDED = "Sharing ended.";
const STALLED = "The shared canvas did not take this change. Your draft is kept.";

interface Shared {
  epoch: string;
  revision: number;
  canonical: CanvasDocument;
  /** The viewport the authority last took from this desktop. */
  viewport: CanvasViewport;
  /** The one unacknowledged op; resent unchanged after a lost reply. */
  inflight?: CanvasAuthorityOp;
  inflightSeen?: boolean;
  /** Close or delete is under way: its own `ended` is not news. */
  closing: boolean;
  resyncing?: Promise<void>;
}

export function createCanvasSessions<F extends CanvasFileRef>(io: CanvasSessionIO<F>) {
  const sessions = new Map<number, CanvasSession<F>>();
  return {
    get: (id: number) => sessions.get(id),
    open(file: F): CanvasSession<F> {
      const existing = sessions.get(file.id);
      if (existing) return existing;
      const key = `oculus:canvas-draft:${file.id}`;
      const authority = io.authority;
      let state: CanvasSnapshot = { text: null, status: "loading", authority: "off" };
      let saved: string | null = null;
      let loading: Promise<void> | undefined;
      let tail = Promise.resolve();
      let draftTail = Promise.resolve();
      let timer: ReturnType<typeof setTimeout> | undefined;
      let writing: Promise<void> | undefined;
      let edits = 0;
      const listeners = new Set<() => void>();
      // Set from the moment the authority is asked to open until the file is reread
      // after sharing; no whole-file write may run while it is.
      let fenced = false;
      let shared: Shared | null = null;
      let attaching: Promise<void> | undefined;
      let buffered: CanvasAuthorityEvent[] | null = null;
      let unsubscribe: (() => void) | undefined;
      let clientId = secureId();
      let seq = 1;
      let parsed: { text: string; document: CanvasDocument } | undefined;

      const publish = (patch: Partial<CanvasSnapshot>) => {
        state = { ...state, ...patch };
        for (const listener of listeners) listener();
      };
      const failed = (error: unknown) => publish({ status: "error", error: errorMessage(error) });
      const run = (operation: () => Promise<void>) => {
        const result = tail.then(operation);
        tail = result.catch(() => {});
        return result;
      };
      const enqueue = (operation: () => Promise<void>) => run(operation).catch((error) => { failed(error); throw error; });
      const draftOperation = (operation: () => void | Promise<void>) => {
        const result = draftTail.then(operation);
        draftTail = result.catch(() => {});
        return result;
      };
      const recoveryFailed = (error: unknown) => publish({ recoveryError: `Recovery copy unavailable: ${errorMessage(error)}` });
      const clearDraft = () => draftOperation(() => io.drafts.removeItem(key))
        .then(() => publish({ recoveryError: undefined }), recoveryFailed);
      const keepDraft = (text: string) => {
        // A shared base never equals the disk, so a draft from a share is restored only by choice.
        const base = shared ? `shared:${shared.epoch}` : saved;
        const record = JSON.stringify({ version: 1, base, text });
        void draftOperation(() => io.drafts.setItem(key, record))
          .then(() => publish({ recoveryError: undefined }), recoveryFailed);
      };
      const writeFile = (text: string) => {
        if (fenced) throw new Error("This canvas is shared; only its authority writes the file.");
        return io.write(session.file, text);
      };
      const local = () => {
        const text = state.text!;
        if (parsed?.text !== text) parsed = { text, document: parseCanvasDocument(text) };
        return parsed.document;
      };
      const show = (document: CanvasDocument) => {
        const text = serializeCanvasDocument(document);
        parsed = { text, document };
        publish({ text });
        if (state.status !== "saved") keepDraft(text);
      };
      const stopListening = () => { unsubscribe?.(); unsubscribe = undefined; };
      const releaseIfIdle = () => {
        if (listeners.size === 0 && !shared && !fenced && !attaching && state.status === "saved" && !state.recovery && sessions.get(file.id) === session) sessions.delete(file.id);
      };

      const conflict = (message: string) => publish({ status: "error", error: message, conflict: message });
      const pause = () => { if (state.authority !== "paused") publish({ authority: "paused", error: PAUSED }); };
      const resume = () => {
        if (state.authority !== "paused") return;
        publish({ authority: "live", error: state.error === PAUSED ? undefined : state.error });
        void session.flush().catch(() => {});
      };
      /** Shows the canonical document with this desktop's unsent content edits on top. */
      const rebase = (s: Shared, before: CanvasDocument, own = false) => {
        if (state.text == null || state.conflict) return;
        const current = local();
        let next: CanvasDocument;
        try {
          const inflight = !s.inflightSeen ? s.inflight?.transaction : undefined;
          const pending = clientPendingTransactions(before, current, inflight);
          next = reconcileClientDocument(s.canonical, own && inflight ? pending.slice(1) : pending, current);
          if (own && s.inflight) s.inflightSeen = true;
        }
        catch (error) { conflict(errorMessage(error)); return; }
        if (next !== current) show(next);
      };
      const advance = (s: Shared, revision: number, transaction: CanvasTransaction, own = false) => {
        const before = s.canonical;
        try { s.canonical = applyCanvasTransaction(before, transaction); }
        catch { void resync(s); return; }
        s.revision = revision;
        rebase(s, before, own);
      };
      const replace = (s: Shared, snapshot: CanvasAuthoritySnapshot) => {
        if (snapshot.epoch !== s.epoch) { leave(s, ENDED); return; }
        if (snapshot.paused) pause();
        if (snapshot.revision < s.revision) return;
        const before = s.canonical;
        s.canonical = parseCanvasDocument(snapshot.text);
        s.revision = snapshot.revision;
        rebase(s, before);
        if (s.inflight && !s.inflightSeen && !state.conflict) s.inflightSeen = containsClientTransaction(s.canonical, s.inflight.transaction);
      };
      const resync = (s: Shared): Promise<void> => {
        s.resyncing ??= authority!.snapshot(session.file).then(
          (snapshot) => { if (shared === s) replace(s, snapshot); },
          (error) => {
            if (shared !== s) return;
            if (authorityCode(error) === "session-ended") leave(s, ENDED); else failed(error);
          },
        ).finally(() => { s.resyncing = undefined; });
        return s.resyncing;
      };
      /** Rereads the file as the whole-file base once the authority no longer writes it. */
      const reopen = async (pending: boolean, since: number, notice?: string) => {
        let disk: string;
        try { disk = await io.read(session.file.relative_path); io.validate?.(disk); }
        finally { fenced = false; }
        saved = disk;
        if (pending && state.text !== disk) {
          publish({ text: disk, status: "saved", recovery: { text: state.text!, valid: true }, error: `${notice ? `${notice} ` : ""}Your latest change was not shared. Choose which version to keep.` });
        } else if (edits !== since) {
          // Edited after sharing stopped, on top of what the file holds: an ordinary save.
          void session.flush().catch(() => {});
        } else {
          await clearDraft();
          publish({ text: disk, status: "saved", error: undefined });
        }
        releaseIfIdle();
      };
      const unshared = (s: Shared) => !!state.conflict || (state.text != null && hasCanvasChanges(diffCanvasContent(s.canonical, local())));
      /** The authority ended this epoch: save through the file again, keeping any unsent change for an explicit choice. */
      const leave = (s: Shared, notice: string) => {
        if (shared !== s) return;
        shared = null;
        stopListening();
        const pending = unshared(s);
        const since = edits;
        publish({ authority: "ended", conflict: undefined });
        void enqueue(() => reopen(pending, since, notice)).catch(() => {});
      };
      const onEvent = (event: CanvasAuthorityEvent) => {
        if (buffered) { buffered.push(event); return; }
        const s = shared;
        if (!s || s.closing || event.fileId !== session.file.id || event.epoch !== s.epoch) return;
        if (event.kind === "state") {
          if (event.phase === "ended") leave(s, ENDED);
          else if (event.phase === "paused") pause();
          else resume();
          return;
        }
        if (event.revision <= s.revision) return;
        if (event.revision === s.revision + 1) advance(s, event.revision, event.transaction, event.clientId === clientId);
        // Our own change past a gap waits for its ack, which checks the revision itself.
        else if (event.clientId !== clientId) void resync(s);
      };
      const acknowledge = async (s: Shared, ack: CanvasAuthorityAck) => {
        if (ack.revision <= s.revision) return;
        if (ack.transaction && ack.revision === s.revision + 1) advance(s, ack.revision, ack.transaction, true);
        else await resync(s);
      };
      /** Decides what a refused or lost commit means; true retries at once. */
      const refused = async (s: Shared, error: unknown): Promise<boolean> => {
        const code = authorityCode(error);
        if (code === "session-ended") { leave(s, errorMessage(error)); return false; }
        if (code === "external-edit") pause();
        if (code === "stale-op") {
          clientId = secureId();
          seq = 1;
          await resync(s);
          return shared === s;
        }
        if (code?.startsWith("conflict-")) conflict(errorMessage(error));
        throw error;
      };
      const commitAll = async (s: Shared) => {
        let previous: CanvasAuthorityOp | undefined;
        while (shared === s) {
          if (state.conflict) throw new Error(state.conflict);
          let op = s.inflight;
          if (!op) {
            const document = local();
            const transaction = diffCanvasContent(s.canonical, document);
            const viewport = sameValue(document.viewport, s.viewport) ? undefined : document.viewport;
            if (!hasCanvasChanges(transaction) && !viewport) break;
            if (previous && sameValue(previous.transaction, transaction) && sameValue(previous.viewport, viewport)) throw new Error(STALLED);
            if (state.authority === "paused") throw new Error(PAUSED);
            op = { clientId, seq, opId: secureId(), transaction, epoch: s.epoch, ...(viewport ? { viewport } : {}) };
            s.inflight = op;
            s.inflightSeen = false;
          } else if (state.authority === "paused") throw new Error(PAUSED);
          publish({ status: "saving", error: undefined });
          let ack: CanvasAuthorityAck;
          try { ack = await authority!.commit(session.file, op); }
          catch (error) {
            if (shared !== s) return;
            if (authorityCode(error)) { s.inflight = undefined; s.inflightSeen = false; }
            if (await refused(s, error)) continue;
            return;
          }
          if (shared !== s) return;
          seq = op.seq + 1;
          if (op.viewport) s.viewport = op.viewport;
          previous = op;
          await acknowledge(s, ack);
          s.inflight = undefined;
          s.inflightSeen = false;
        }
        if (shared !== s) return;
        await clearDraft();
        publish({ status: "saved", error: undefined });
      };

      const probeAuthority = async () => {
        if (!authority?.existing) return null;
        buffered = [];
        unsubscribe = authority.onRemote(session.file.id, onEvent);
        fenced = true;
        try {
          const snapshot = await authority.existing(session.file);
          if (!snapshot) { buffered = null; stopListening(); fenced = false; }
          return snapshot;
        } catch (error) { buffered = null; stopListening(); fenced = false; throw error; }
      };
      const joinAuthority = (snapshot: CanvasAuthoritySnapshot, before?: CanvasDocument) => {
        const canonical = parseCanvasDocument(snapshot.text);
        const s: Shared = { epoch: snapshot.epoch, revision: snapshot.revision, canonical, viewport: canonical.viewport, closing: false };
        shared = s;
        publish({ authority: "live" });
        if (snapshot.paused) pause();
        if (before) rebase(s, before);
        const early = buffered!;
        buffered = null;
        for (const event of early) onEvent(event);
      };

      const readRecovery = async (disk: string) => {
        let raw: string | null;
        try { raw = await io.drafts.getItem(key); }
        catch (error) { publish({ text: disk, status: "saved", error: undefined }); recoveryFailed(error); return; }
        if (raw == null) { publish({ text: disk, status: "saved", error: undefined }); return; }
        let draft = raw;
        let base: string | undefined;
        try {
          const value = JSON.parse(raw) as { version?: number; text?: string; base?: string };
          if (value.version === 1 && typeof value.text === "string" && typeof value.base === "string") { draft = value.text; base = value.base; }
        } catch { /* A legacy draft needs an explicit recovery decision. */ }
        let valid = true;
        try { io.validate?.(draft); } catch { valid = false; }
        if (draft === disk) { await clearDraft(); publish({ text: disk, status: "saved", error: undefined }); return; }
        if (valid && base === disk) {
          publish({ text: draft, status: "unsaved", error: undefined });
          void session.flush().catch(() => {});
        } else {
          publish({ text: disk, status: "saved", recovery: { text: draft, valid }, error: valid ? "The file changed since the recovery copy was made. Choose which version to keep." : "The local recovery copy is invalid. It has been preserved; the file on disk is unchanged." });
        }
      };

      const save = async () => {
        if (state.status === "deleted" || state.text == null) return;
        if (state.recovery) throw new Error("Choose which canvas version to keep before saving.");
        if (shared) return commitAll(shared);
        // Sharing is opening or closing; that step saves when it finishes.
        if (fenced) return;
        while (saved !== state.text) {
          const text = state.text;
          publish({ status: "saving", error: undefined });
          await writeFile(text);
          saved = text;
        }
        // A draft is cleared only after the latest revision reaches disk.
        await clearDraft();
        publish({ status: "saved", error: undefined });
      };
      const session: CanvasSession<F> = {
        file,
        get authority() { return state.authority; },
        snapshot: () => state,
        subscribe(listener) { listeners.add(listener); return () => { listeners.delete(listener); }; },
        load() {
          if (loading) return loading;
          loading = (async () => {
            // Native sharing outlives a webview and its JavaScript sessions. Subscribe
            // before probing it, and never open an ordinary writer beside that owner.
            const existing = await probeAuthority();
            const disk = existing?.text ?? await io.read(session.file.relative_path);
            io.validate?.(disk);
            saved = disk;
            await readRecovery(disk);
            if (existing) joinAuthority(existing);
          })().catch((error) => { loading = undefined; if (!shared) { buffered = null; stopListening(); fenced = false; } failed(error); throw error; });
          return loading;
        },
        update(text) {
          if (state.text === null || state.status === "deleted" || state.recovery || state.conflict || text === state.text) return;
          io.validate?.(text);
          edits += 1;
          publish({ text, status: "unsaved", error: state.authority === "paused" ? state.error : undefined });
          keepDraft(text);
          clearTimeout(timer);
          timer = setTimeout(() => { void session.flush().catch(() => {}); }, io.delay ?? 500);
        },
        flush() {
          clearTimeout(timer);
          // An edit that lands while a save winds down is saved by the same flush: the
          // check and the clearing of `writing` happen in one step, so no flush gets a finished save.
          writing ??= enqueue(async () => {
            try { let seen; do { seen = edits; await save(); } while (seen !== edits); }
            finally { writing = undefined; }
          });
          return writing;
        },
        rename(title) {
          clearTimeout(timer);
          return enqueue(async () => {
            await save();
            session.file = shared ? await authority!.rename(session.file, title) : await io.rename(session.file, title);
            await save();
            publish({});
          });
        },
        release() {
          void tail.then(() => loading).then(releaseIfIdle).catch(() => {});
        },
        recoverDraft() {
          const recovery = state.recovery;
          if (!recovery?.valid) return;
          publish({ recovery: undefined, error: undefined });
          session.update(recovery.text);
        },
        discardDraft() {
          void enqueue(async () => {
            await clearDraft();
            publish({ recovery: undefined, error: undefined, status: "saved" });
          }).catch(() => {});
        },
        remove() {
          clearTimeout(timer);
          return enqueue(async () => {
            const s = shared;
            if (s) {
              s.closing = true;
              let ended = false;
              try { await authority!.remove(session.file); }
              catch (error) {
                if (authorityCode(error) !== "session-ended") { s.closing = false; throw error; }
                ended = true;
              }
              shared = null;
              stopListening();
              fenced = false;
              // The authority had already let go of the file, so it is still on disk.
              if (ended) await io.remove(session.file);
            } else {
              await io.remove(session.file);
            }
            await clearDraft();
            saved = state.text;
            publish({ status: "deleted", error: undefined, conflict: undefined, ...(s ? { authority: "ended" as const } : {}) });
          });
        },
        attachAuthority() {
          if (!authority) return Promise.reject(new Error("Sharing is not available here."));
          if (shared) return Promise.resolve();
          attaching ??= run(async () => {
            if (shared) return;
            await session.load();
            if (shared) return;
            if (state.status === "deleted") throw new Error("This canvas was deleted.");
            const existing = await probeAuthority();
            if (existing) {
              joinAuthority(existing, parseCanvasDocument(saved!));
              void session.flush().catch(() => {});
              return;
            }
            if (state.recovery) throw new Error("Choose which canvas version to keep before sharing.");
            publish({ authority: "attaching" });
            try {
              try { await save(); } catch (error) { failed(error); throw error; }
              const sent = state.text!;
              buffered = [];
              unsubscribe = authority.onRemote(session.file.id, onEvent);
              fenced = true;
              const snapshot = await authority.open(session.file, sent);
              const canonical = parseCanvasDocument(snapshot.text);
              const s: Shared = { epoch: snapshot.epoch, revision: snapshot.revision, canonical, viewport: canonical.viewport, closing: false };
              shared = s;
              publish({ authority: "live" });
              if (snapshot.paused) pause();
              // Edits made while the authority opened, or a share that was already live, meet here.
              rebase(s, parseCanvasDocument(sent));
              const early = buffered;
              buffered = null;
              for (const event of early) onEvent(event);
              void session.flush().catch(() => {});
            } catch (error) {
              buffered = null;
              stopListening();
              shared = null;
              fenced = false;
              publish({ authority: "off" });
              throw error;
            }
          }).finally(() => { attaching = undefined; });
          return attaching;
        },
        detachAuthority() {
          clearTimeout(timer);
          return run(async () => {
            const s = shared;
            if (!s) return;
            // What cannot be shared stays a draft for the recovery choice below.
            await commitAll(s).catch(() => {});
            if (shared !== s) return;
            s.closing = true;
            const since = edits;
            const pending = unshared(s);
            try { await authority!.close(session.file); } catch (error) { s.closing = false; throw error; }
            if (shared !== s) return;
            shared = null;
            stopListening();
            publish({ authority: "off", conflict: undefined });
            await reopen(pending, since);
          });
        },
        useSharedCanvas() {
          return enqueue(async () => {
            const s = shared;
            if (!s) return;
            // Its fate is unknown, so its seq is never reused for another op.
            if (s.inflight) { s.inflight = undefined; seq += 1; }
            const snapshot = await authority!.snapshot(session.file);
            if (shared !== s) return;
            if (snapshot.epoch !== s.epoch) { leave(s, ENDED); return; }
            s.canonical = parseCanvasDocument(snapshot.text);
            s.revision = snapshot.revision;
            const viewport = local().viewport;
            await clearDraft();
            publish({ conflict: undefined, error: undefined, status: "saved" });
            show({ ...s.canonical, viewport });
            await commitAll(s);
          });
        },
        resolveExternal(choice) {
          return enqueue(async () => {
            const s = shared;
            if (!s) return;
            const snapshot = await authority!.resolve(session.file, choice);
            if (shared !== s) return;
            if (state.authority === "paused") publish({ authority: "live", error: state.error === PAUSED ? undefined : state.error });
            replace(s, snapshot);
            await commitAll(s);
          });
        },
      };
      sessions.set(file.id, session);
      return session;
    },
  };
}
