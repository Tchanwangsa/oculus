/** One serialized write queue per file, shared by every mounted pane. */
export interface CanvasFileRef { id: number; relative_path: string }
export interface CanvasSnapshot {
  text: string | null;
  status: "loading" | "saved" | "unsaved" | "saving" | "error" | "deleted";
  error?: string;
  recovery?: { text: string; valid: boolean };
}
export interface CanvasSessionIO<F extends CanvasFileRef> {
  read(path: string): Promise<string>;
  write(file: F, text: string): Promise<void>;
  rename(file: F, title: string): Promise<F>;
  remove(file: F): Promise<void>;
  drafts: Pick<Storage, "getItem" | "setItem" | "removeItem">;
  delay?: number;
  validate?(text: string): void;
}
export interface CanvasSession<F extends CanvasFileRef> {
  file: F;
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
}

export function createCanvasSessions<F extends CanvasFileRef>(io: CanvasSessionIO<F>) {
  const sessions = new Map<number, CanvasSession<F>>();
  return {
    open(file: F): CanvasSession<F> {
      const existing = sessions.get(file.id);
      if (existing) return existing;
      const key = `oculus:canvas-draft:${file.id}`;
      let state: CanvasSnapshot = { text: null, status: "loading" };
      let saved: string | null = null;
      let loading: Promise<void> | undefined;
      let tail = Promise.resolve();
      let timer: ReturnType<typeof setTimeout> | undefined;
      let writing: Promise<void> | undefined;
      const listeners = new Set<() => void>();
      const publish = (patch: Partial<CanvasSnapshot>) => {
        state = { ...state, ...patch };
        for (const listener of listeners) listener();
      };
      const failed = (error: unknown) => publish({ status: "error", error: String(error) });
      const enqueue = (operation: () => Promise<void>) => {
        const result = tail.then(operation);
        tail = result.catch(() => {});
        return result.catch((error) => { failed(error); throw error; });
      };
      const save = async () => {
        if (state.status === "deleted" || state.text == null) return;
        if (state.recovery) throw new Error("Choose which canvas version to keep before saving.");
        while (saved !== state.text) {
          const text = state.text;
          publish({ status: "saving", error: undefined });
          await io.write(session.file, text);
          saved = text;
        }
        // A draft is cleared only after the latest revision reaches disk.
        io.drafts.removeItem(key);
        publish({ status: "saved", error: undefined });
      };
      const session: CanvasSession<F> = {
        file,
        snapshot: () => state,
        subscribe(listener) { listeners.add(listener); return () => { listeners.delete(listener); }; },
        load() {
          if (loading) return loading;
          loading = (async () => {
            const disk = await io.read(session.file.relative_path);
            io.validate?.(disk);
            saved = disk;
            const raw = io.drafts.getItem(key);
            if (raw == null) { publish({ text: disk, status: "saved", error: undefined }); return; }
            let draft = raw;
            let base: string | undefined;
            try {
              const value = JSON.parse(raw) as { version?: number; text?: string; base?: string };
              if (value.version === 1 && typeof value.text === "string" && typeof value.base === "string") { draft = value.text; base = value.base; }
            } catch { /* A legacy draft needs an explicit recovery decision. */ }
            let valid = true;
            try { io.validate?.(draft); } catch { valid = false; }
            if (draft === disk) { io.drafts.removeItem(key); publish({ text: disk, status: "saved", error: undefined }); return; }
            if (valid && base === disk) {
              publish({ text: draft, status: "unsaved", error: undefined });
              void session.flush().catch(() => {});
            } else {
              publish({ text: disk, status: "saved", recovery: { text: draft, valid }, error: valid ? "The file changed since the recovery copy was made. Choose which version to keep." : "The local recovery copy is invalid. It has been preserved; the file on disk is unchanged." });
            }
          })().catch((error) => { loading = undefined; failed(error); throw error; });
          return loading;
        },
        update(text) {
          if (state.text === null || state.status === "deleted" || state.recovery || text === state.text) return;
          io.validate?.(text);
          publish({ text, status: "unsaved", error: undefined });
          try { io.drafts.setItem(key, JSON.stringify({ version: 1, base: saved, text })); } catch (error) { failed(`Local recovery copy failed: ${String(error)}`); }
          clearTimeout(timer);
          timer = setTimeout(() => { void session.flush().catch(() => {}); }, io.delay ?? 500);
        },
        flush() {
          clearTimeout(timer);
          if (!writing) {
            writing = enqueue(save).finally(() => { writing = undefined; });
          }
          return writing;
        },
        rename(title) {
          clearTimeout(timer);
          return enqueue(async () => {
            await save();
            session.file = await io.rename(session.file, title);
            await save();
            publish({});
          });
        },
        release() {
          void tail.then(() => loading).then(() => {
            if (listeners.size === 0 && state.status === "saved" && !state.recovery && sessions.get(file.id) === session) sessions.delete(file.id);
          }).catch(() => {});
        },
        recoverDraft() {
          const recovery = state.recovery;
          if (!recovery?.valid) return;
          publish({ recovery: undefined, error: undefined });
          session.update(recovery.text);
        },
        discardDraft() {
          io.drafts.removeItem(key);
          publish({ recovery: undefined, error: undefined, status: "saved" });
        },
        remove() {
          clearTimeout(timer);
          return enqueue(async () => {
            await io.remove(session.file);
            io.drafts.removeItem(key);
            saved = state.text;
            publish({ status: "deleted", error: undefined });
          });
        },
      };
      sessions.set(file.id, session);
      return session;
    },
  };
}
