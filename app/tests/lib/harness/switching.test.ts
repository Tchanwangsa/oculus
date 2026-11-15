import { expect, test } from "bun:test";
import { handoffLabel, runSwitch, switchLocked, type Capabilities, type HarnessRun, type Provider } from "@/lib/harness";

const caps = (handoff: Provider[]) => (provider: Provider) => ({ provider, providerHandoff: handoff.includes(provider) }) as Capabilities;

const run = (id: number, provider: Provider, model: string | null, effort: string | null): HarnessRun => ({
  id, status: "completed", delivery: "delivered", answer_item_id: null, started_ms: null, ended_ms: null, provider, model, effort,
});

test("a started chat moves only to a bridge that takes a handoff", () => {
  const taking = caps(["claude", "codex"]);
  expect(switchLocked("claude", "codex", taking)).toBe(false);
  expect(switchLocked("claude", "opencode", taking)).toBe(true);
  // Its own provider is never locked, and nothing is before the chat starts.
  expect(switchLocked("opencode", "opencode", taking)).toBe(false);
  expect(switchLocked(null, "antigravity", taking)).toBe(false);
});

test("a divider names only what changed between two runs", () => {
  const first = run(1, "claude", "opus", "high");
  expect(runSwitch(first, run(2, "claude", "opus", "high"))).toBeNull();
  expect(runSwitch(first, run(2, "claude", "sonnet", "high"))).toEqual({ model: ["opus", "sonnet"] });
  expect(runSwitch(first, run(2, "claude", "opus", null))).toEqual({ effort: ["high", null] });
  expect(runSwitch(first, run(2, "codex", "gpt", "high"))).toEqual({ provider: ["claude", "codex"], model: ["opus", "gpt"] });
  // The CLI resolving an alias is not the student changing model.
  expect(runSwitch({ ...first, resolved_model: "claude-opus-5-5" }, run(2, "claude", "opus", "high"))).toBeNull();
  // A run from before selections were recorded says nothing either way.
  const old: HarnessRun = { ...first, provider: undefined };
  expect(runSwitch(old, first)).toBeNull();
  expect(runSwitch(undefined, first)).toBeNull();
});

test("a handoff reads as pending, done, or failed in danger tone", () => {
  const handoff = { kind: "full", from: "claude", omitted: 0 } as const;
  expect(handoffLabel({ ...handoff, status: "pending" })).toEqual({ label: "Context handoff pending", tone: "neutral" });
  expect(handoffLabel({ ...handoff, status: "inline" })).toEqual({ label: "Context handoff", tone: "neutral" });
  expect(handoffLabel({ ...handoff, status: "failed" })).toEqual({ label: "Context handoff", tone: "danger" });
});
