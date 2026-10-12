import { expect, test } from "bun:test";
import { AGENT_ORDER, agentOption, orderAgents } from "@/lib/onboarding/agentOptions";

test("nothing ready keeps the neutral order", () => {
  expect(orderAgents(() => false)).toEqual([...AGENT_ORDER]);
});

test("ready agents move to the front, each half in order", () => {
  const ready = new Set(["antigravity", "codex"]);
  expect(orderAgents((p) => ready.has(p))).toEqual(["codex", "antigravity", "claude", "opencode"]);
});

test("an installed opencode sorts with the ready agents", () => {
  const ready = new Set(["opencode"]);
  expect(orderAgents((p) => ready.has(p))).toEqual(["opencode", "claude", "codex", "antigravity"]);
});

test("every agent has a cost line and an https link", () => {
  for (const p of AGENT_ORDER) {
    const o = agentOption(p);
    expect(o.provider).toBe(p);
    expect(o.cost.length).toBeGreaterThan(0);
    expect(o.link.url.startsWith("https://")).toBe(true);
  }
});
