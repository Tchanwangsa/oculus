import Database from "@tauri-apps/plugin-sql";
import { createWidgetStore, widgetKey } from "../../src/stores/chat/widgetStore";
import { buildRows } from "../../src/components/harness/timeline/rows";
import { useHarnessStore } from "../../src/stores/chat/harnessStore";
import { runFromRow, type RunRow } from "../../src/lib/harness/runs";
import type { HarnessEnvelope, HarnessItem } from "../../src/lib/harness/types";

const input = await Bun.stdin.json();
const thread = input.envelopes[0].threadId;
useHarnessStore.setState({ items: { [thread]: [] }, holds: { [thread]: 1 }, live: {}, threads: [], queued: {} });
const steps = [];
for (const envelope of input.envelopes as HarnessEnvelope[]) {
  useHarnessStore.getState().apply(envelope);
  useHarnessStore.getState().flushLive();
  const state = useHarnessStore.getState();
  steps.push({ items: state.items[thread], live: state.live[thread] });
}
if (input.repeatEnvelope) {
  const envelope = input.envelopes.findLast((e: HarnessEnvelope) => e.event.type === "assistant_message");
  if (envelope) useHarnessStore.getState().apply(envelope);
}
const state = useHarnessStore.getState();
const rows = buildRows(state.items[thread] ?? [], state.live[thread]?.running ?? false, { runs: state.runs[thread] });
// A reload reads the runs beside the rows (`getHarnessRuns`).
const storedRuns = Object.fromEntries((input.runs as RunRow[]).map((r) => [r.id, runFromRow(r)]));
const reopened = buildRows(input.reopened as HarnessItem[], false, { runs: storedRuns });
const entries = new Map<string, string>();
const storage = { getItem: (key: string) => entries.get(key) ?? null, setItem: (key: string, value: string) => { entries.set(key, value); } } as Storage;
const widgets = createWidgetStore(storage);
for (const item of state.items[thread] ?? []) {
  const source = item.content?.match(/```widget\n([\s\S]*?)\n```/)?.[1];
  if (source) widgets.getState().save(widgetKey(thread, item.id, 1), source, { progress: 1 });
}
const widgetKeys = (input.reopened as HarnessItem[]).filter((item) => item.content?.includes("```widget")).map((item) => widgetKey(thread, item.id, 1));
const restored = createWidgetStore(storage).getState().entries;
const artifactStateStable = widgetKeys.every((key) => restored[key]?.state.progress === 1 && restored[key]?.ask == null);
let racedItems: HarnessItem[] | null = null;
if (input.loadRace) {
  let resolveRead!: (items: HarnessItem[]) => void;
  const read = new Promise<HarnessItem[]>((resolve) => { resolveRead = resolve; });
  // Substitute the SQL plugin transport; the production reads and load reducer run.
  Database.load = (async (path: string) => {
    if (path !== "sqlite:oculus.db") throw new Error(`unrecorded SQL open: ${path}`);
    return { select: async (query: string, values: unknown[]) => {
      if (values[0] !== thread) throw new Error(`unrecorded SQL read: ${query}`);
      // The runs are read beside the rows; the race is over the rows.
      if (query.includes("FROM harness_runs WHERE thread_id = $1")) return [];
      if (query !== "SELECT * FROM harness_items WHERE thread_id = $1 ORDER BY id ASC") throw new Error(`unrecorded SQL read: ${query}`);
      return read;
    } };
  }) as typeof Database.load;
  (globalThis as any).window = { __TAURI_INTERNALS__: { invoke: async (command: string) => {
    if (command !== "harness_queued") throw new Error(`unrecorded native operation: ${command}`);
    return [];
  } } };
  useHarnessStore.setState({ items: { [thread]: [] }, holds: { [thread]: 1 }, live: {}, threads: [], queued: {} });
  const cut = Math.max(1, input.envelopes.findIndex((e: HarnessEnvelope) => e.event.type === "assistant_message"));
  // A fresh store over the same stream: the first pass already saw these
  // sequence numbers, so the race replays them without.
  const race = (input.envelopes as HarnessEnvelope[]).map(({ sequence: _, ...e }) => e as HarnessEnvelope);
  for (const envelope of race.slice(0, cut)) useHarnessStore.getState().apply(envelope);
  const loading = useHarnessStore.getState().load(thread);
  for (const envelope of race.slice(cut)) useHarnessStore.getState().apply(envelope);
  resolveRead(input.snapshots[cut - 1]);
  await loading;
  useHarnessStore.getState().flushLive();
  racedItems = useHarnessStore.getState().items[thread];
}
process.stdout.write(JSON.stringify({ steps, rows, reopened, items: state.items[thread], live: state.live[thread], artifactStateStable, racedItems }));
