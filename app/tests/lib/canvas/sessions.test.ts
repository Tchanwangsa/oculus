import { describe, expect, test } from "bun:test";
import { createCanvasSessions, type CanvasFileRef, type CanvasSessionIO } from "@/lib/canvas/sessions";

const tick = () => new Promise((resolve) => setTimeout(resolve, 0));
function deferred() { let resolve!: () => void; const promise = new Promise<void>((r) => { resolve = r; }); return { promise, resolve }; }
function harness(initial = "disk", overrides: Partial<CanvasSessionIO<CanvasFileRef>> = {}) {
  const drafts = new Map<string, string>();
  const files = new Map([["courses/X/canvases/Ideas.canvas", initial]]);
  const writes: { path: string; text: string; done: ReturnType<typeof deferred> }[] = [];
  let fail = false;
  const file: CanvasFileRef = { id: 1, relative_path: "courses/X/canvases/Ideas.canvas" };
  const sessions = createCanvasSessions({
    drafts: { getItem: (key) => drafts.get(key) ?? null, setItem: (key, text) => { drafts.set(key, text); }, removeItem: (key) => { drafts.delete(key); } },
    delay: 60_000,
    read: async (path) => { if (!files.has(path)) throw new Error("missing"); return files.get(path)!; },
    write: async (ref, text) => {
      const done = deferred(); writes.push({ path: ref.relative_path, text, done }); await done.promise;
      if (fail) throw new Error("disk full");
      files.set(ref.relative_path, text);
    },
    rename: async (ref, title) => { const moved = ref.relative_path.replace(/[^/]+$/, `${title}.canvas`); files.set(moved, files.get(ref.relative_path)!); files.delete(ref.relative_path); return { ...ref, relative_path: moved }; },
    remove: async (ref) => { files.delete(ref.relative_path); },
    validate: (text) => { if (text.startsWith("invalid")) throw new Error("Invalid drawing"); },
    ...overrides,
  });
  return { file, drafts, files, writes, sessions, setFail(value: boolean) { fail = value; } };
}

describe("canvas sessions", () => {
  test("panes share edits and writes preserve latest revision in order", async () => {
    const h = harness(); const session = h.sessions.open(h.file); await session.load();
    expect(h.sessions.open(h.file)).toBe(session);
    session.update("one"); await tick();
    expect(JSON.parse(h.drafts.values().next().value!).text).toBe("one");
    const flush = session.flush(); await tick();
    session.update("two"); expect(session.flush()).toBe(flush);
    expect(h.writes.map((w) => w.text)).toEqual(["one"]);
    h.writes[0].done.resolve(); await tick();
    expect(h.writes.map((w) => w.text)).toEqual(["one", "two"]);
    h.writes[1].done.resolve(); await flush;
    expect(session.snapshot().status).toBe("saved");
    expect(h.drafts.size).toBe(0);
    expect(h.files.get(h.file.relative_path)).toBe("two");
  });

  test("rename drains old writes and changes the target of later edits", async () => {
    const h = harness(); const session = h.sessions.open(h.file); await session.load();
    session.update("before"); const flush = session.flush(); await tick();
    const rename = session.rename("Plan");
    h.writes[0].done.resolve(); await flush; await rename;
    session.update("after"); const next = session.flush(); await tick();
    expect(h.writes[1].path).toBe("courses/X/canvases/Plan.canvas");
    h.writes[1].done.resolve(); await next;
    expect(h.files.has(h.file.relative_path)).toBe(false);
    expect(h.files.get(session.file.relative_path)).toBe("after");
  });

  test("save failure retains durable draft and retry succeeds", async () => {
    const h = harness(); const session = h.sessions.open(h.file); await session.load();
    h.setFail(true); session.update("retained"); const failed = session.flush(); await tick();
    h.writes[0].done.resolve(); await expect(failed).rejects.toThrow("disk full");
    expect(session.snapshot().status).toBe("error"); expect(h.drafts.size).toBe(1);
    h.setFail(false); const retry = session.flush(); await tick(); h.writes[1].done.resolve(); await retry;
    expect(h.files.get(h.file.relative_path)).toBe("retained"); expect(h.drafts.size).toBe(0);
  });

  test("recovery checks base and validates before overwriting disk", async () => {
    const h = harness("external");
    h.drafts.set("oculus:canvas-draft:1", JSON.stringify({ version: 1, base: "disk", text: "recovered" }));
    const session = h.sessions.open(h.file); await session.load();
    expect(session.snapshot().text).toBe("external"); expect(session.snapshot().recovery?.valid).toBe(true);
    await expect(session.flush()).rejects.toThrow("Choose"); expect(h.writes.length).toBe(0);
    session.recoverDraft(); const flush = session.flush(); await tick(); h.writes[0].done.resolve(); await flush;
    expect(h.files.get(h.file.relative_path)).toBe("recovered");

    const invalid = harness(); invalid.drafts.set("oculus:canvas-draft:1", JSON.stringify({ version: 1, base: "disk", text: "invalid drawing" }));
    const bad = invalid.sessions.open(invalid.file); await bad.load(); bad.recoverDraft();
    expect(bad.snapshot().recovery?.valid).toBe(false); expect(bad.snapshot().text).toBe("disk");
    expect(invalid.writes.length).toBe(0); expect(invalid.drafts.size).toBe(1);
  });

  test("matching-base recovery autosaves; deletion never recreates the file", async () => {
    const h = harness(); h.drafts.set("oculus:canvas-draft:1", JSON.stringify({ version: 1, base: "disk", text: "draft" }));
    const session = h.sessions.open(h.file); await session.load(); await tick();
    expect(h.writes[0].text).toBe("draft"); h.writes[0].done.resolve(); await session.flush();
    session.update("pending"); await session.remove(); await session.flush();
    expect(h.files.has(h.file.relative_path)).toBe(false); expect(h.writes.length).toBe(1); expect(h.drafts.size).toBe(0);
  });

  test("async recovery cannot reappear after the latest drawing is saved", async () => {
    const gate = deferred();
    const drafts = new Map<string, string>();
    const h = harness('disk', { drafts: {
      getItem: async key => drafts.get(key) ?? null,
      setItem: async (key, text) => { await gate.promise; drafts.set(key, text); },
      removeItem: async key => { drafts.delete(key); },
    } });
    const session = h.sessions.open(h.file); await session.load();
    session.update('first');
    const saved = session.flush(); await tick();
    h.writes[0].done.resolve(); await tick();
    session.update('latest');
    gate.resolve(); await tick();
    expect(h.writes[1].text).toBe('latest');
    h.writes[1].done.resolve(); await saved;
    expect(h.files.get(h.file.relative_path)).toBe('latest');
    expect(drafts.size).toBe(0);
    expect(session.snapshot().status).toBe('saved');
  });

  test("recovery storage failure does not mark a successful document save as failed", async () => {
    const h = harness('disk', { drafts: {
      getItem: async () => null,
      setItem: async () => { throw new Error('recovery unavailable'); },
      removeItem: async () => {},
    } });
    const session = h.sessions.open(h.file); await session.load();
    session.update('new drawing'); await tick();
    expect(session.snapshot().status).toBe('unsaved');
    expect(session.snapshot().recoveryError).toContain('recovery unavailable');
    const saved = session.flush(); await tick(); h.writes[0].done.resolve(); await saved;
    expect(session.snapshot().status).toBe('saved');
    expect(session.snapshot().error).toBeUndefined();
    expect(h.files.get(h.file.relative_path)).toBe('new drawing');
  });

});
