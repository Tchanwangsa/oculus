import { afterAll, beforeEach, describe, expect, test } from "bun:test";
import type { BridgeHealth } from "../src/lib/harness";
import { claudeWslUnavailableReason, useHarnessHealthStore } from "../src/stores/harnessHealthStore";

const ready: BridgeHealth = {
  provider: "claude",
  label: "Claude Code via WSL2",
  path: "Oculus:/home/oculus/.local/bin/claude",
  version: "2.0.0",
  error: null,
  overrideEnv: "OCULUS_CLAUDE_WSL_DISTRO",
};

const previousWindow = globalThis.window;
let check: () => Promise<BridgeHealth[]>;
let calls = 0;

beforeEach(() => {
  calls = 0;
  check = async () => [ready];
  Object.assign(globalThis, {
    window: {
      __TAURI_INTERNALS__: {
        invoke: (command: string) => {
          expect(command).toBe("harness_health");
          calls++;
          return check();
        },
      },
    },
  });
  useHarnessHealthStore.setState({ health: null, checking: false, error: null, checkedAt: 0 });
});

afterAll(() => { Object.assign(globalThis, { window: previousWindow }); });

describe("Claude WSL availability", () => {
  test("blocks until a successful bridge probe finds a launch path", () => {
    expect(claudeWslUnavailableReason(null, null)).toContain("Checking");
    expect(claudeWslUnavailableReason([], null)).toContain("not ready");
    expect(claudeWslUnavailableReason([{ ...ready, path: null }], null)).toContain("not ready");
    expect(claudeWslUnavailableReason([ready], null)).toBeUndefined();
  });

  test("retains actionable setup failures even when Claude was found", () => {
    const error = "Sign in to Claude Code inside the Oculus WSL2 distribution.";
    expect(claudeWslUnavailableReason([{ ...ready, error }], null)).toBe(error);
    expect(claudeWslUnavailableReason([ready], "WSL probe failed")).toContain("WSL probe failed");
  });

  test("a recheck publishes recovery to all health subscribers", async () => {
    check = async () => [{ ...ready, error: "Claude is not signed in." }];
    await useHarnessHealthStore.getState().refresh();
    let state = useHarnessHealthStore.getState();
    expect(claudeWslUnavailableReason(state.health, state.error)).toContain("not signed in");

    check = async () => [ready];
    await useHarnessHealthStore.getState().refresh(true);
    state = useHarnessHealthStore.getState();
    expect(claudeWslUnavailableReason(state.health, state.error)).toBeUndefined();
    expect(calls).toBe(2);
  });

  test("probe transport errors block stale ready state and can recover", async () => {
    await useHarnessHealthStore.getState().refresh();
    check = async () => { throw new Error("probe timed out"); };
    await useHarnessHealthStore.getState().refresh(true);
    let state = useHarnessHealthStore.getState();
    expect(state.health).toEqual([ready]);
    expect(state.checking).toBe(false);
    expect(claudeWslUnavailableReason(state.health, state.error)).toContain("probe timed out");

    check = async () => [ready];
    await useHarnessHealthStore.getState().refresh(true);
    state = useHarnessHealthStore.getState();
    expect(state.error).toBeNull();
    expect(claudeWslUnavailableReason(state.health, state.error)).toBeUndefined();
  });

  test("mount and focus checks coalesce without replacing in-flight health", async () => {
    let finish!: (health: BridgeHealth[]) => void;
    check = () => new Promise((resolve) => { finish = resolve; });
    const first = useHarnessHealthStore.getState().refresh();
    await useHarnessHealthStore.getState().refresh();
    await useHarnessHealthStore.getState().refresh(true);
    expect(calls).toBe(1);
    expect(useHarnessHealthStore.getState().checking).toBe(true);
    finish([ready]);
    await first;
    await useHarnessHealthStore.getState().refresh();
    expect(calls).toBe(1);
    expect(useHarnessHealthStore.getState().checking).toBe(false);
  });
});
