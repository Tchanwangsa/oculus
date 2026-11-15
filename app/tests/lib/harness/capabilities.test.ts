import { expect, test } from "bun:test";
import { createCapabilities, type Capabilities } from "@/lib/harness/capabilities";

const codex: Capabilities = {
  provider: "codex", interrupt: true, rewind: true, sharedProcess: true, activeSteering: false, steerByRestart: false,
  modelSwitchInSession: true, providerHandoff: true, approvals: true, autoReview: true, sessionGrants: true,
  modeSwitchInSession: true,
};

test("every optional action reads as unavailable until Rust has answered", async () => {
  let answer!: (rows: Capabilities[]) => void;
  let asked = 0;
  const caps = createCapabilities(() => {
    asked += 1;
    return new Promise((resolve) => (answer = resolve));
  });
  let told = 0;
  caps.subscribe(() => (told += 1));
  const before = caps.of("codex");
  expect(before).toEqual({
    ...codex, interrupt: false, rewind: false, sharedProcess: false, modelSwitchInSession: false, providerHandoff: false,
    approvals: false, autoReview: false, sessionGrants: false, modeSwitchInSession: false,
  });
  // The same object while nothing has changed, as `useSyncExternalStore` needs.
  expect(caps.of("codex")).toBe(before);
  expect(asked).toBe(1);

  answer([codex]);
  await caps.load();
  expect(told).toBe(1);
  expect(caps.of("codex")).toBe(codex);
  // A provider Rust did not list keeps the conservative answer.
  expect(caps.of("antigravity").rewind).toBe(false);
  expect(asked).toBe(1);
});

test("a failed read is asked again by the next reader", async () => {
  let fail = true;
  const caps = createCapabilities(async () => {
    if (fail) throw new Error("Tauri is not available");
    return [codex];
  });
  await expect(caps.load()).rejects.toThrow("Tauri is not available");
  // Reading asks again; that try fails too and leaves the answer unknown.
  expect(caps.of("codex").rewind).toBe(false);
  await caps.load().catch(() => {});
  fail = false;
  await caps.load();
  expect(caps.of("codex").rewind).toBe(true);
});
