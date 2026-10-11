import { expect, test } from "bun:test";
import { gateFromRow, isAgentReady, isExistingSetup, type AgentProbe } from "@/lib/onboarding/gate";

const agent = (over: Partial<AgentProbe> & Pick<AgentProbe, "provider">): AgentProbe => ({
  installed: false,
  signIn: "unknown",
  ...over,
});

test("the row alone decides when it is present", () => {
  expect(gateFromRow({ completedAt: "2026-10-10T00:00:00.000Z", skipped: [] })).toBe("app");
  expect(gateFromRow({ completedAt: null, skipped: ["library"] })).toBe("onboarding");
  expect(gateFromRow(null)).toBe("probe");
});

test("an agent is ready when installed and signed in", () => {
  expect(isAgentReady(agent({ provider: "claude", installed: true, signIn: "in" }))).toBe(true);
  expect(isAgentReady(agent({ provider: "claude", installed: true, signIn: "out" }))).toBe(false);
  expect(isAgentReady(agent({ provider: "codex", installed: false, signIn: "in" }))).toBe(false);
  // A failed check reads unknown, which is not signed in.
  expect(isAgentReady(agent({ provider: "antigravity", installed: true }))).toBe(false);
});

test("opencode counts as ready once installed, since its sign-in is unanswerable", () => {
  expect(isAgentReady(agent({ provider: "opencode", installed: true }))).toBe(true);
  expect(isAgentReady(agent({ provider: "opencode", installed: false }))).toBe(false);
});

test("an existing setup needs Canvas and one ready agent", () => {
  const ready = [
    agent({ provider: "claude", installed: true, signIn: "out" }),
    agent({ provider: "codex", installed: true, signIn: "in" }),
  ];
  expect(isExistingSetup(true, ready)).toBe(true);
  expect(isExistingSetup(false, ready)).toBe(false);
  expect(isExistingSetup(true, [agent({ provider: "claude", installed: true, signIn: "out" })])).toBe(false);
  expect(isExistingSetup(true, [])).toBe(false);
});
