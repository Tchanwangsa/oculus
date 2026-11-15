import { expect, test } from "bun:test";
import type { Selection } from "@/lib/harness";
import { createPickStore } from "@/stores/chat/pickStore";

const tick = () => new Promise((r) => setTimeout(r, 0));

function rig(answer: (s: Selection) => Promise<void> = () => Promise.resolve()) {
  const saves: Selection[] = [];
  const stored: Selection[] = [];
  const store = createPickStore(
    (_thread, s) => {
      saves.push(s);
      return answer(s);
    },
    (_thread, s) => void stored.push(s),
  );
  return { store, saves, stored };
}

const claude: Selection = { provider: "claude", model: "claude-opus-5-5", effort: "high" };

test("a same-provider pick saves at once, as one save for a model and its level", async () => {
  const { store, saves, stored } = rig();
  store.getState().pick(1, claude, { model: "claude-sonnet-5-5" });
  store.getState().pick(1, claude, { effort: "medium" });
  // The composer names it before Rust has answered.
  expect(store.getState().held[1]).toEqual({ provider: "claude", model: "claude-sonnet-5-5", effort: "medium" });
  await tick();
  expect(saves).toEqual([{ provider: "claude", model: "claude-sonnet-5-5", effort: "medium" }]);
  expect(stored).toEqual(saves);
  expect(store.getState().held[1]).toBeUndefined();
});

test("another provider's pick is held until its catalogue names a model, then saved", async () => {
  const { store, saves } = rig();
  store.getState().pick(1, claude, { provider: "codex" });
  await tick();
  expect(saves).toEqual([]);
  expect(store.getState().held[1]).toEqual({ provider: "codex", model: null, effort: null });
  // The composer fills it from Codex's list.
  store.getState().pick(1, claude, { model: "gpt-6.1-sol" });
  store.getState().pick(1, claude, { effort: "high" });
  await tick();
  expect(saves).toEqual([{ provider: "codex", model: "gpt-6.1-sol", effort: "high" }]);
});

test("a refusal keeps Rust's sentence and the chat's own selection", async () => {
  const reason = "Codex cannot take over a chat that has started. Start a new chat to switch.";
  const { store, stored } = rig(() => Promise.reject(reason));
  store.getState().pick(1, claude, { provider: "codex", model: "gpt-6.1-sol" });
  await tick();
  expect(store.getState().held[1]).toBeUndefined();
  expect(store.getState().refused[1]).toBe(reason);
  expect(stored).toEqual([]);
  // The next pick clears it.
  store.getState().pick(1, claude, { effort: "max" });
  expect(store.getState().refused[1]).toBeUndefined();
});

test("an answer to an older pick does not undo a newer one", async () => {
  let reject!: (e: unknown) => void;
  const answers = [new Promise<void>((_, r) => (reject = r)), Promise.resolve()];
  const { store } = rig(() => answers.shift()!);
  store.getState().pick(1, claude, { model: "claude-sonnet-5-5" });
  await tick();
  store.getState().pick(1, claude, { effort: "low" });
  reject("too late");
  await tick();
  expect(store.getState().refused[1]).toBeUndefined();
  expect(store.getState().held[1]).toBeUndefined();
});

test("a send carries what was held, and nothing stays held", () => {
  const { store, stored } = rig();
  store.getState().pick(1, claude, { provider: "codex" });
  store.getState().sent(1);
  expect(stored).toEqual([{ provider: "codex", model: null, effort: null }]);
  expect(store.getState().held).toEqual({});
});
