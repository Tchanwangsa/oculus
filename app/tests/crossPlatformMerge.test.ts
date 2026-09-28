import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { resolve } from "node:path";
import { chatHref, chatThreadId, type HarnessItem } from "../src/lib/harness";
import { itemsFor, useHarnessStore } from "../src/stores/harnessStore";

// OS decisions happen at module load. Separate processes exercise the actual
// entry modules without a cached Windows import leaking into the Mac case.
for (const platform of ["MacIntel", "Win32"]) {
  test(`${platform}: native defaults, database location and live Claude catalogue coexist`, () => {
    const script = `
      Object.defineProperty(globalThis, "navigator", { value: { platform: ${JSON.stringify(platform)} }, configurable: true });
      const calls = [];
      let databaseUrl;
      globalThis.window = { __TAURI_INTERNALS__: { invoke: async (command, args) => {
        calls.push(command);
        if (command === "library_database_url") return "sqlite:C:/library/oculus.db";
        if (command === "plugin:sql|load") { databaseUrl = args.db; return args.db; }
        if (command === "plugin:sql|select") return [{ value: JSON.stringify({ threadNaming: {
          provider: "antigravity", model: "saved-model", reasoningEffort: "high"
        } }) }];
        if (command === "harness_claude_models") return [
          { value: "default", resolvedModel: "claude-sonnet-4-6", displayName: "Default", description: "Sonnet 4.6 · Balanced", supportedEffortLevels: ["high", "low", "medium"] },
          { value: "sonnet", resolvedModel: "claude-sonnet-4-6", displayName: "Sonnet", description: "Sonnet 4.6 · Balanced" },
          { value: "haiku", resolvedModel: "claude-haiku-4-5-20251001", displayName: "Haiku", description: "Haiku 4.5 · Fast", supportedEffortLevels: [] }
        ];
        throw new Error("Unexpected IPC: " + command);
      } } };
      const h = await import("./src/lib/harness.ts");
      const db = await import("./src/lib/db.ts");
      const p = await import("./src/lib/platform.ts");
      const { useHarnessStore } = await import("./src/stores/harnessStore.ts");
      const claude = h.providerInfo("claude");
      const models = await claude.fetchModels();
      const jobs = await db.getJobModels();
      console.log(JSON.stringify({
        databaseUrl, calls, isMac: p.isMac, isWindows: p.isWindows,
        shortcut: p.shortcut("B", true), enter: p.shortcut("Enter"),
        provider: useHarnessStore.getState().provider,
        naming: db.DEFAULT_JOB_MODELS.threadNaming, saved: jobs.threadNaming,
        claudeLabel: claude.label, staticModels: claude.staticModels,
        selection: h.defaultSelection(models), models,
        rewind: h.PROVIDERS.map(({ id, rewind }) => [id, rewind]),
      }));
    `;
    const result = Bun.spawnSync([process.execPath, "--eval", script], {
      cwd: resolve(import.meta.dir, ".."),
      stderr: "pipe",
      stdout: "pipe",
    });
    expect(result.exitCode).toBe(0);
    expect(result.stderr.toString()).toBe("");
    const actual = JSON.parse(result.stdout.toString());
    const windows = platform === "Win32";
    expect(actual.isMac).toBe(!windows);
    expect(actual.isWindows).toBe(windows);
    expect(actual.shortcut).toBe(windows ? "Ctrl+Alt+B" : "⌥⌘B");
    expect(actual.enter).toBe(windows ? "Ctrl+Enter" : "⌘↵");
    expect(actual.databaseUrl).toBe(windows ? "sqlite:C:/library/oculus.db" : "sqlite:oculus.db");
    expect(actual.calls.filter((c: string) => c === "library_database_url")).toHaveLength(windows ? 1 : 0);
    expect(actual.provider).toBe(windows ? "codex" : "claude");
    expect(actual.naming).toEqual(windows
      ? { provider: "codex", model: "gpt-5.6-luna", reasoningEffort: "low" }
      : { provider: "claude", model: "claude-haiku-4-5-20251001", reasoningEffort: null });
    expect(actual.saved).toEqual({ provider: "antigravity", model: "saved-model", reasoningEffort: "high" });
    expect(actual.claudeLabel).toBe(windows ? "Claude Code via WSL2" : "Claude Code");
    expect(actual.staticModels).toBeNull();
    expect(actual.models.map((m: { id: string }) => m.id)).toEqual(["claude-sonnet-4-6", "claude-haiku-4-5-20251001"]);
    expect(actual.models[0].reasoningEfforts).toEqual(["low", "medium", "high"]);
    expect(actual.models[1].defaultReasoningEffort).toBeNull();
    expect(actual.selection).toEqual({ model: "claude-sonnet-4-6", reasoning: "medium" });
    expect(actual.rewind).toEqual([["claude", true], ["codex", true], ["opencode", true], ["antigravity", false]]);
  });
}

describe("conversations stay independent across Chat tabs and lecture docks", () => {
  const initial = useHarnessStore.getState();
  const previousWindow = globalThis.window;
  let reads: number[];
  let read: (id: number) => Promise<HarnessItem[]>;
  const row = (id: number): HarnessItem => ({
    id: id * 10, thread_id: id, kind: "user", ref_id: null,
    content: `Thread ${id}`, meta: null, created_at: "2026-09-28T00:00:00Z",
  });

  beforeEach(() => {
    reads = [];
    read = async (id) => [row(id)];
    useHarnessStore.getState().flushLive();
    useHarnessStore.setState(initial, true);
    Object.assign(globalThis, { window: { __TAURI_INTERNALS__: {
      invoke: async (command: string, args: { db: string; values: number[] }) => {
        if (command === "library_database_url") return "sqlite::memory:";
        if (command === "plugin:sql|load") return args.db;
        if (command === "plugin:sql|select") {
          reads.push(args.values[0]);
          return read(args.values[0]);
        }
        if (command === "harness_queued") return [];
        throw new Error(`Unexpected IPC: ${command}`);
      },
    } } });
  });
  afterEach(() => {
    useHarnessStore.getState().flushLive();
    useHarnessStore.setState(initial, true);
    Object.assign(globalThis, { window: previousWindow });
  });

  test("routes retain separate conversation identities and reject invalid ids", () => {
    const first = chatHref(7, "Lecture & quiz / review");
    const second = chatHref(9, "Assignment");
    expect(chatThreadId(first.slice(first.indexOf("?")))).toBe(7);
    expect(chatThreadId(second.slice(second.indexOf("?")))).toBe(9);
    expect(new URLSearchParams(first.split("?")[1]).get("n")).toBe("Lecture & quiz / review");
    expect(chatHref()).toBe("/chat");
    for (const search of ["", "?t=0", "?t=-1", "?t=1.5", "?t=unknown"]) expect(chatThreadId(search)).toBeNull();
  });

  test("live responses reach both held timelines without changing the other thread", async () => {
    const store = useHarnessStore.getState();
    await Promise.all([store.hold(7), store.hold(9)]);
    store.apply({ threadId: 7, itemId: 71, event: { type: "assistant_message", text: "First reply" } });
    store.apply({ threadId: 9, itemId: 91, event: { type: "assistant_message", text: "Second reply" } });
    const items = useHarnessStore.getState().items;
    expect(itemsFor(7, items).map((i) => i.content)).toEqual(["Thread 7", "First reply"]);
    expect(itemsFor(9, items).map((i) => i.content)).toEqual(["Thread 9", "Second reply"]);
    expect(itemsFor(null, items)).toEqual([]);
  });

  test("a dock cannot release a conversation still held by either Chat tab", async () => {
    const store = useHarnessStore.getState();
    await store.hold(7);
    await store.hold(7);
    expect(reads).toEqual([7]);
    store.unhold(7);
    store.release(7);
    expect(useHarnessStore.getState().items[7]).toEqual([row(7)]);
    store.unhold(7);
    store.release(7);
    expect(useHarnessStore.getState().items[7]).toBeUndefined();
  });

  test("releasing a loading dock does not resurrect its timeline when SQLite finishes", async () => {
    let finish: ((rows: HarnessItem[]) => void) | undefined;
    read = () => new Promise((resolve) => { finish = resolve; });
    const store = useHarnessStore.getState();
    const loading = store.load(7);
    // getDb may resolve from a previous test; wait for the mocked read itself.
    for (let n = 0; n < 10 && !finish; n++) await Bun.sleep(0);
    expect(finish).toBeDefined();
    store.release(7);
    finish!([row(7)]);
    await loading;
    expect(useHarnessStore.getState().items[7]).toBeUndefined();
    expect(useHarnessStore.getState().queued[7]).toBeUndefined();
  });
});
