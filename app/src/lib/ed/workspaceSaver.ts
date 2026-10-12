/**
 * The save loop of one challenge workspace, free of React and Tauri so it
 * can be tested. Each file read into the editor gets a buffer: the text as
 * typed and the text last read or written. An edit schedules a write
 * {@link SAVE_DELAY_MS} after typing stops; writes go one at a time across
 * the workspace (one upload in flight, as Ed rate-limits) and loop until
 * every buffer matches what was written. A failed write stops the loop and
 * shows; the next edit or flush tries again. Same shape as
 * `lib/notes/documentSessions.ts`.
 */

/** How long after the last keystroke a file is written. */
export const SAVE_DELAY_MS = 600;

export type SaveState =
  | { state: "idle" }
  | { state: "saving" }
  /** `where`: uploaded to Ed, or kept on disk while offline. */
  | { state: "saved"; where: "ed" | "local" }
  | { state: "error"; message: string };

export interface SaverListener {
  status?(status: SaveState): void;
  /** A buffer's text changed: an edit, or someone else's text taken. */
  text?(path: string, text: string): void;
}

export interface SaverIO {
  write(path: string, text: string): Promise<"ed" | "local">;
  delay?: number;
}

interface Buffer {
  text: string;
  /** The text last read from or written to disk. */
  disk: string;
}

export type WorkspaceSaver = ReturnType<typeof createWorkspaceSaver>;

export function createWorkspaceSaver(io: SaverIO) {
  const delay = io.delay ?? SAVE_DELAY_MS;
  const buffers = new Map<string, Buffer>();
  const listeners = new Set<SaverListener>();
  let status: SaveState = { state: "idle" };
  let timer: ReturnType<typeof setTimeout> | null = null;
  let writing: Promise<void> | null = null;

  const isDirty = (b: Buffer | undefined) => b != null && b.text !== b.disk;
  const nextDirty = () => [...buffers].find(([, b]) => isDirty(b));

  function emit(next: SaveState) {
    status = next;
    for (const l of listeners) l.status?.(next);
  }

  function notify(path: string, text: string) {
    for (const l of listeners) l.text?.(path, text);
  }

  /** Write every dirty buffer now. Idempotent while a write is running. */
  function flush(): Promise<void> {
    if (timer != null) {
      clearTimeout(timer);
      timer = null;
    }
    if (writing) return writing;
    if (!nextDirty()) return Promise.resolve();
    const run = (async () => {
      try {
        let where: "ed" | "local" | null = null;
        for (let entry = nextDirty(); entry; entry = nextDirty()) {
          const [path, buffer] = entry;
          const text = buffer.text;
          emit({ state: "saving" });
          try {
            where = await io.write(path, text);
          } catch (e) {
            emit({ state: "error", message: String(e) });
            return;
          }
          // The buffer may have been dropped or replaced meanwhile.
          const now = buffers.get(path);
          if (now) now.disk = text;
        }
        if (where) emit({ state: "saved", where });
      } finally {
        writing = null;
      }
    })();
    writing = run;
    return run;
  }

  function schedule() {
    if (timer != null) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = null;
      void flush();
    }, delay);
  }

  return {
    get status() {
      return status;
    },
    /** The buffer's text, or undefined until the file is read. */
    text: (path: string): string | undefined => buffers.get(path)?.text,
    /** True when `path` (any file, without one) has unsaved text. */
    dirty: (path?: string): boolean => (path == null ? !!nextDirty() : isDirty(buffers.get(path))),
    /** A read landed; a buffer that already exists keeps its text. */
    loaded(path: string, text: string): string {
      const had = buffers.get(path);
      if (had) return had.text;
      buffers.set(path, { text, disk: text });
      return text;
    },
    /** An editor's change: keep it and schedule the write. */
    edit(path: string, text: string) {
      const buffer = buffers.get(path);
      if (!buffer || buffer.text === text) return;
      buffer.text = text;
      notify(path, text);
      schedule();
    },
    /** Someone else changed the file: take their text unless ours is unsaved.
     *  Returns whether it was taken. */
    external(path: string, text: string): boolean {
      const buffer = buffers.get(path);
      if (!buffer) return false;
      if (isDirty(buffer)) return false;
      buffer.disk = text;
      if (buffer.text !== text) {
        buffer.text = text;
        notify(path, text);
      }
      return true;
    },
    /** Drops the buffers with nothing unsaved, except those in `keep`, so the
     *  next open reads them from disk again. */
    forgetClean(keep: (path: string) => boolean = () => false) {
      for (const [path, buffer] of buffers) {
        if (!isDirty(buffer) && !keep(path)) buffers.delete(path);
      }
    },
    flush,
    subscribe(listener: SaverListener): () => void {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}
