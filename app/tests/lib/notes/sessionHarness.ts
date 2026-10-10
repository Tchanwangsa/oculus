import { history } from "@codemirror/commands";
import { EditorState, type Transaction, type TransactionSpec } from "@codemirror/state";

import {
  createDocumentSessions,
  type DocumentLease,
  type NoteRef,
  type SessionListener,
} from "@/lib/notes/documentSessions";

export type Note = NoteRef & { filename: string };

export function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

export const tick = (ms = 0) => new Promise((resolve) => setTimeout(resolve, ms));

/** A disk whose reads and writes resolve only when the test says so. */
export function fakeDisk(initial: Record<string, string>) {
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
export class FakeView {
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
export function withState(state: EditorState) {
  const target = { state, dispatch: (tr: Transaction) => void (target.state = tr.state) };
  return target;
}

export function listener(log: string[] = []): SessionListener & { log: string[] } {
  return {
    log,
    status: (s) => log.push(s.state),
    saved: (text) => log.push(`saved:${text}`),
  };
}

export const NOTE: Note = { id: 1, relative_path: "courses/X/documents/a.md", filename: "a.md" };

export async function opened(sessions: ReturnType<typeof createDocumentSessions<Note>>, disk: ReturnType<typeof fakeDisk>) {
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
