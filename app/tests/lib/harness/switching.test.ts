import { expect, test } from "bun:test";
import {
  answerModel,
  handoffLabel,
  pendingSwitch,
  pickSaves,
  pickSelection,
  questionProvider,
  runDivider,
  runSwitch,
  switchLocked,
  switchSides,
  threadSelection,
  type Capabilities,
  type HarnessRun,
  type ModelName,
  type Provider,
} from "@/lib/harness";

const caps = (handoff: Provider[]) => (provider: Provider) => ({ provider, providerHandoff: handoff.includes(provider) }) as Capabilities;

const run = (id: number, provider: Provider, model: string | null, effort: string | null): HarnessRun => ({
  id, status: "completed", delivery: "delivered", answer_item_id: null, started_ms: null, ended_ms: null, provider, model, effort,
});

const NAMES: Record<string, string> = { "claude-opus-5-5": "Opus 5.5", "claude-sonnet-5-5": "Sonnet 5.5", "gpt-6.1-sol": "GPT-6.1 Sol" };
const name: ModelName = (_provider, model) => NAMES[model] ?? model;

test("a started chat moves only to a bridge that takes a handoff", () => {
  const taking = caps(["claude", "codex"]);
  expect(switchLocked("claude", "codex", taking)).toBe(false);
  expect(switchLocked("claude", "opencode", taking)).toBe(true);
  // Its own provider is never locked, and nothing is before the chat starts.
  expect(switchLocked("opencode", "opencode", taking)).toBe(false);
  expect(switchLocked(null, "antigravity", taking)).toBe(false);
  // Today no bridge has a recording: every other tab of a started chat is locked.
  const none = caps([]);
  expect((["codex", "opencode", "antigravity"] as Provider[]).every((p) => switchLocked("claude", p, none))).toBe(true);
});

test("a divider names only what changed between two runs", () => {
  const first = run(1, "claude", "opus", "high");
  expect(runSwitch(first, run(2, "claude", "opus", "high"))).toBeNull();
  expect(runSwitch(first, run(2, "claude", "sonnet", "high"))).toEqual({ model: ["opus", "sonnet"] });
  expect(runSwitch(first, run(2, "claude", "opus", "max"))).toEqual({ effort: ["high", "max"] });
  expect(runSwitch(first, run(2, "codex", "gpt", "high"))).toEqual({ provider: ["claude", "codex"], model: ["opus", "gpt"] });
  // The CLI resolving an alias is not the student changing model.
  expect(runSwitch({ ...first, resolved_model: "claude-opus-5-5" }, run(2, "claude", "opus", "high"))).toBeNull();
  // A run from before levels were stored has none: no "— → High" divider.
  expect(runSwitch(run(1, "claude", "opus", null), first)).toBeNull();
  expect(runSwitch(first, run(2, "claude", "opus", null))).toBeNull();
  // A run from before selections were recorded says nothing either way.
  const old: HarnessRun = { ...first, provider: undefined };
  expect(runSwitch(old, first)).toBeNull();
  expect(runSwitch(undefined, first)).toBeNull();
});

test("a divider's sides name the agent only where it changed, and models by their names", () => {
  const owners: [Provider, Provider] = ["claude", "claude"];
  expect(switchSides({ model: ["claude-opus-5-5", "claude-sonnet-5-5"] }, name, owners)).toEqual(["Opus 5.5", "Sonnet 5.5"]);
  expect(switchSides({ effort: ["high", "max"] }, name, owners)).toEqual(["High reasoning", "Max reasoning"]);
  expect(switchSides({ model: ["claude-opus-5-5", "claude-sonnet-5-5"], effort: ["high", "medium"] }, name, owners))
    .toEqual(["Opus 5.5 · High", "Sonnet 5.5 · Medium"]);
  expect(switchSides({ provider: ["claude", "codex"], model: ["claude-opus-5-5", "gpt-6.1-sol"] }, name, ["claude", "codex"]))
    .toEqual(["Claude Code · Opus 5.5", "Codex · GPT-6.1 Sol"]);
  // An id no catalogue names is shown as it is.
  expect(switchSides({ model: ["claude-opus-5-5", "claude-new"] }, name, owners)).toEqual(["Opus 5.5", "claude-new"]);
});

test("a run's divider joins its switch and its handoff in one line", () => {
  const before = run(1, "claude", "claude-opus-5-5", "high");
  const switched: HarnessRun = { ...run(2, "codex", "gpt-6.1-sol", "high"), handoff: { kind: "full", status: "inline", from: "claude", omitted: 0 } };
  expect(runDivider(before, switched, name)).toEqual({
    label: "Claude Code · Opus 5.5 → Codex · GPT-6.1 Sol", detail: "Context handoff", tone: "neutral", title: "Codex · gpt-6.1-sol", switched: true,
  });
  // A handoff that was never delivered is drawn in the danger tone, with what was left out.
  const failed: HarnessRun = { ...switched, handoff: { kind: "full", status: "failed", from: "claude", omitted: 3 } };
  expect(runDivider(before, failed, name)).toMatchObject({ detail: "Context handoff, 3 earlier messages left out", tone: "danger" });
  // A handoff within one provider (a session that had to start over) has no switch.
  const fresh: HarnessRun = { ...run(2, "claude", "claude-opus-5-5", "high"), handoff: { kind: "full", status: "pending", from: null, omitted: 1 } };
  expect(runDivider(before, fresh, name)).toMatchObject({ label: "Context handoff pending", detail: "1 earlier message left out", switched: false });
  expect(runDivider(before, run(2, "claude", "claude-opus-5-5", "high"), name)).toBeNull();
});

test("a handoff reads as pending, done, or failed in danger tone", () => {
  const handoff = { kind: "full", from: "claude", omitted: 0 } as const;
  expect(handoffLabel({ ...handoff, status: "pending" })).toEqual({ label: "Context handoff pending", tone: "neutral" });
  expect(handoffLabel({ ...handoff, status: "inline" })).toEqual({ label: "Context handoff", tone: "neutral" });
  expect(handoffLabel({ ...handoff, status: "failed" })).toEqual({ label: "Context handoff", tone: "danger" });
});

test("an answer's footer names its own run's model, and an old run's shows none", () => {
  const codex = run(2, "codex", "gpt-6.1-sol", "high");
  expect(answerModel(codex, name)).toEqual({ label: "GPT-6.1 Sol", title: "Codex · gpt-6.1-sol · High reasoning" });
  // What the CLI reported running wins over what was asked for.
  const resolved: HarnessRun = { ...run(1, "claude", "opus", null), resolved_model: "claude-opus-5-5" };
  expect(answerModel(resolved, name)).toEqual({ label: "Opus 5.5", title: "Claude Code · claude-opus-5-5" });
  // Runs from before models were recorded, and rows with no run, keep the time alone.
  expect(answerModel({ ...codex, provider: undefined, model: undefined }, name)).toBeNull();
  expect(answerModel(run(3, "claude", null, null), name)).toBeNull();
  expect(answerModel(undefined, name)).toBeNull();
});

test("a pick of another provider starts empty and is held until it names a model", () => {
  const current = { provider: "claude" as Provider, model: "claude-opus-5-5", effort: "high" };
  const moved = pickSelection(current, { provider: "codex" });
  expect(moved).toEqual({ provider: "codex", model: null, effort: null });
  expect(pickSaves(moved)).toBe(false);
  // The composer fills it from Codex's catalogue: model, then level, each a save.
  const filled = pickSelection(pickSelection(moved, { model: "gpt-6.1-sol" }), { effort: "medium" });
  expect(filled).toEqual({ provider: "codex", model: "gpt-6.1-sol", effort: "medium" });
  expect(pickSaves(filled)).toBe(true);
  // A same-provider pick keeps what it does not name, and saves at once.
  expect(pickSelection(current, { effort: "max" })).toEqual({ ...current, effort: "max" });
  expect(pickSaves(pickSelection(current, { model: "claude-sonnet-5-5" }))).toBe(true);
  // Naming its own provider is not a switch.
  expect(pickSelection(current, { provider: "claude" })).toEqual(current);
});

test("the composer shows a held pick over the thread's selection, and the thread's level over the dial", () => {
  const thread = { provider: "claude" as Provider, model: "claude-opus-5-5", effort: "high" };
  expect(threadSelection(thread, undefined, "low")).toEqual(thread);
  // A thread from before levels were stored reads the session's dial.
  expect(threadSelection({ ...thread, effort: null }, undefined, "low").effort).toBe("low");
  const held = { provider: "codex" as Provider, model: null, effort: null };
  expect(threadSelection(thread, held, "low")).toBe(held);
});

test("a pick mid-turn waits for the next send, and the running run keeps its own", () => {
  const running: HarnessRun = { ...run(4, "claude", "claude-opus-5-5", "high"), status: "running" };
  const runs = { 3: run(3, "claude", "claude-opus-5-5", "high"), 4: running };
  const pick = pickSelection({ provider: "claude", model: "claude-opus-5-5", effort: "high" }, { provider: "codex", model: "gpt-6.1-sol" });
  // It is stored now (Rust keeps it for the next send)...
  expect(pickSaves(pick)).toBe(true);
  // ...and is a switch still to come, from the provider of the run answering now...
  expect(pendingSwitch(pick, runs)).toBe("claude");
  // ...whose footer still names its own model.
  expect(answerModel(running, name)?.label).toBe("Opus 5.5");
  // No switch waits once the newest run went out under the pick.
  expect(pendingSwitch(pick, { ...runs, 5: run(5, "codex", "gpt-6.1-sol", null) })).toBeNull();
  expect(pendingSwitch(pick, {})).toBeNull();
});

test("a question's actions follow its own run's provider", () => {
  expect(questionProvider(run(1, "codex", "gpt-6.1-sol", null), "claude")).toBe("codex");
  expect(questionProvider(undefined, "claude")).toBe("claude");
  expect(questionProvider(undefined, undefined)).toBeNull();
});
