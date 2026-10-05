import { historyField } from "@codemirror/commands";
import {
  Annotation,
  EditorState,
  Transaction,
  type EditorStateConfig,
  type TransactionSpec,
} from "@codemirror/state";

import type { SaveStatus } from "@/components/documents/DocumentControls";
import { readCourseFile } from "@/lib/courseFiles";
import type { DbFile } from "@/lib/db";
import { saveDocument } from "@/lib/documents";

/**
 * One editing session per open note, shared by every editor showing it — a
 * side panel, a background tab, the same editor remounted by Fast Refresh.
 * The session owns the text and its writes: the first editor reads the file,
 * later ones take the session's text without a read, and every write goes
 * through one serialised loop (one in flight, looping until disk matches),
 * so no read races a write and no copy saves over another.
 *
 * Views stay identical by replaying each local change into the others with
 * {@link syncedEdit}, outside their undo history (CodeMirror's split-view
 * pattern); selection, scroll and undo stay per view. A view that goes
 * (Fast Refresh remounts the editor) leaves its undo history and caret with
 * the session, and the next view starts from them, so a remount never empties
 * ⌘Z. A session ends once no editor holds it and nothing is unsaved — a tick
 * later, so a detach and re-attach in the same tick keeps it. A failed save
 * keeps it alive with the unsaved text. Free of React so Fast Refresh rarely
 * re-evaluates it.
 */

/** How long after the last keystroke the text goes to disk. */
export const SAVE_DELAY_MS = 600;

/** Marks a change replayed from another view of the same note. */
export const syncedEdit = Annotation.define<boolean>();

/** Enough of a row to write it: the id keys the session (a rename keeps it),
 *  the path is where the next write goes. */
export interface NoteRef {
  id: number;
  relative_path: string;
}

/** The part of an `EditorView` a session drives. */
export interface SessionView {
  readonly state: EditorState;
  dispatch(spec: TransactionSpec): void;
}

/** One editor's view of the session's saves. */
export interface SessionListener {
  status(status: SaveStatus): void;
  /** A write landed: the text now on disk, and when. */
  saved(text: string, at: number): void;
}

export interface SessionIO<F extends NoteRef> {
  read(relativePath: string): Promise<string>;
  write(file: F, text: string): Promise<void>;
  delay?: number;
  /** The clock for `savedAt`. */
  now?: () => number;
}

export interface Session<F extends NoteRef> {
  readonly id: number;
  file: F;
  /** The current text; null until the first read lands. */
  text: string | null;
  /** The text last read from or written to disk. */
  disk: string;
  loading: Promise<void> | null;
  timer: ReturnType<typeof setTimeout> | null;
  writing: Promise<void> | null;
  /** A rename in progress; writes wait for it so they land at the new path. */
  moving: Promise<void> | null;
  listeners: Set<SessionListener>;
  views: Set<SessionView>;
  status: SaveStatus;
  savedAt: number | null;
  /** The last view's state (text, caret, undo history) as JSON, taken when it
   *  detached; {@link DocumentLease.restore} uses it while the text still
   *  matches. */
  stash: { doc?: string } | null;
}

/** One editor's hold on a note's session. */
export interface DocumentLease<F extends NoteRef> {
  /** The text now; null until {@link load} resolves on a first open. */
  readonly text: string | null;
  /** The row writes go to, moved by {@link rename}. */
  readonly file: F;
  readonly savedText: string;
  readonly savedAt: number | null;
  readonly dirty: boolean;
  load(): Promise<string>;
  /** The state for a new view: the session's text, with the caret and undo
   *  history the previous view left if the text is still what it saw. */
  restore(config: EditorStateConfig): EditorState;
  /** Attach a view made from {@link restore}; returns the detach. */
  bind(view: SessionView): () => void;
  /** From the view's update listener: replay its changes, schedule a save. */
  changed(view: SessionView, transactions: readonly Transaction[]): void;
  flush(): Promise<void>;
  /** Save, then move the note with writes held; false (nothing moved) if
   *  the save failed. */
  rename(move: (file: F) => Promise<F>): Promise<boolean>;
  release(): void;
}

const replayed = [syncedEdit.of(true), Transaction.addToHistory.of(false)];

export function createDocumentSessions<F extends NoteRef>(
  io: SessionIO<F>,
  sessions: Map<number, Session<F>> = new Map(),
) {
  const delay = io.delay ?? SAVE_DELAY_MS;
  const now = io.now ?? Date.now;
  const dirty = (s: Session<F>) => s.text !== null && s.text !== s.disk;

  function emit(s: Session<F>, status: SaveStatus) {
    s.status = status;
    for (const l of s.listeners) l.status(status);
  }

  /** Write the text if it differs from disk. Idempotent. */
  function flush(s: Session<F>): Promise<void> {
    if (s.timer != null) {
      clearTimeout(s.timer);
      s.timer = null;
    }
    if (s.writing) return s.writing;
    if (!dirty(s)) return Promise.resolve();
    const run = (async () => {
      try {
        while (dirty(s)) {
          if (s.moving) {
            await s.moving;
            continue;
          }
          const text = s.text as string;
          emit(s, { state: "saving" });
          try {
            await io.write(s.file, text);
          } catch (e) {
            // Not retried here: the next edit schedules another attempt.
            emit(s, { state: "error", message: String(e) });
            break;
          }
          s.disk = text;
          const at = now();
          s.savedAt = at;
          for (const l of s.listeners) l.saved(text, at);
          emit(s, { state: "saved" });
        }
      } finally {
        s.writing = null;
        settle(s);
      }
    })();
    s.writing = run;
    return run;
  }

  function schedule(s: Session<F>) {
    if (s.timer != null) clearTimeout(s.timer);
    s.timer = setTimeout(() => {
      s.timer = null;
      void flush(s);
    }, delay);
  }

  /** Drop the session a tick after it is idle and unheld. */
  function settle(s: Session<F>) {
    if (s.listeners.size || s.views.size) return;
    setTimeout(() => {
      const idle =
        !s.listeners.size && !s.views.size && !s.writing && !s.loading && s.timer == null;
      if (!idle || dirty(s) || sessions.get(s.id) !== s) return;
      sessions.delete(s.id);
    }, 0);
  }

  function load(s: Session<F>): Promise<string> {
    if (s.text !== null) return Promise.resolve(s.text);
    s.loading ??= io
      .read(s.file.relative_path)
      .then((text) => {
        if (s.text === null) {
          s.text = text;
          s.disk = text;
        }
      })
      .finally(() => {
        s.loading = null;
        settle(s);
      });
    return s.loading.then(() => s.text as string);
  }

  function open(file: F, listener: SessionListener): DocumentLease<F> {
    let s = sessions.get(file.id);
    if (!s) {
      s = {
        id: file.id,
        file,
        text: null,
        disk: "",
        loading: null,
        timer: null,
        writing: null,
        moving: null,
        listeners: new Set(),
        views: new Set(),
        status: { state: "idle" },
        savedAt: null,
        stash: null,
      };
      sessions.set(file.id, s);
    }
    const session = s;
    session.listeners.add(listener);
    listener.status(session.status);
    let released = false;

    return {
      get text() {
        return session.text;
      },
      get file() {
        return session.file;
      },
      get savedText() {
        return session.disk;
      },
      get savedAt() {
        return session.savedAt;
      },
      get dirty() {
        return dirty(session);
      },
      load: () => load(session),
      restore(config) {
        const saved = session.stash;
        if (saved && saved.doc === session.text) {
          try {
            return EditorState.fromJSON(saved, config, { history: historyField });
          } catch {
            // An unreadable stash is only a lost undo history.
          }
        }
        return EditorState.create({ doc: session.text ?? "", ...config });
      },
      bind(view) {
        const text = session.text;
        if (text !== null && view.state.doc.toString() !== text) {
          view.dispatch({
            changes: { from: 0, to: view.state.doc.length, insert: text },
            annotations: replayed,
          });
        }
        session.views.add(view);
        return () => {
          session.views.delete(view);
          session.stash = view.state.toJSON({ history: historyField });
          settle(session);
        };
      },
      changed(view, transactions) {
        const local = transactions.filter((tr) => tr.docChanged && !tr.annotation(syncedEdit));
        if (!local.length) return;
        session.text = view.state.doc.toString();
        for (const other of session.views) {
          if (other === view) continue;
          for (const tr of local) other.dispatch({ changes: tr.changes, annotations: replayed });
        }
        schedule(session);
      },
      flush: () => flush(session),
      async rename(move) {
        await flush(session);
        if (dirty(session)) return false;
        let done = () => {};
        session.moving = new Promise<void>((resolve) => (done = resolve));
        try {
          session.file = await move(session.file);
          return true;
        } finally {
          session.moving = null;
          done();
          if (dirty(session) && !session.writing) schedule(session);
        }
      },
      release() {
        if (released) return;
        released = true;
        session.listeners.delete(listener);
        // The last editor leaving writes now rather than at the debounce.
        if (!session.listeners.size) void flush(session);
        settle(session);
      },
    };
  }

  /** Write every session's unsaved text now. */
  function flushAll(): Promise<void> {
    return Promise.all([...sessions.values()].map(flush)).then(() => {});
  }

  return { open, flushAll, sessions };
}

// Kept across hot updates of this module, so editors remounted onto the new
// code join the old sessions (and their in-flight writes) instead of reading.
const hot = import.meta.hot;
const shared: Map<number, Session<DbFile>> = hot?.data.documentSessions ?? new Map();
if (hot) hot.data.documentSessions = shared;

export const documentSessions = createDocumentSessions<DbFile>(
  {
    read: readCourseFile,
    write: saveDocument,
  },
  shared,
);

// A reload or quit drops anything not yet written; best effort, since the
// page may go before the write's reply comes back.
if (typeof window !== "undefined") {
  const flushAll = () => void documentSessions.flushAll();
  window.addEventListener("pagehide", flushAll);
  window.addEventListener("beforeunload", flushAll);
  hot?.dispose(() => {
    window.removeEventListener("pagehide", flushAll);
    window.removeEventListener("beforeunload", flushAll);
  });
}
