import { afterAll, expect, mock, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";

const calls: { command: string; args: Record<string, unknown> }[] = [];
// The rest of the module stays real: later test files import it too.
const tauri = { ...(await import("@tauri-apps/api/core")) };
mock.module("@tauri-apps/api/core", () => ({
  ...tauri,
  invoke: async (command: string, args: Record<string, unknown>) => {
    calls.push({ command, args });
    return null;
  },
}));
afterAll(() => {
  calls.length = 0;
});

const { useThreadActions } = await import("@/components/harness/threads/useThreadActions");
type Actions = ReturnType<typeof useThreadActions>;

/** The thread's actions, read by rendering the hook once on the server. */
function actionsFor(threadId: number | null): Actions {
  let out: Actions | null = null;
  function Probe() {
    out = useThreadActions({
      threadId: () => threadId,
      selection: { provider: "codex", model: "gpt", effort: "high" },
      onRestore: () => {},
      onCreated: () => {},
    });
    return null;
  }
  renderToStaticMarkup(<Probe />);
  return out!;
}

test("Send now writes the waiting message into the running turn by its id", async () => {
  await actionsFor(7).pending.steerQueued("42");
  const sent = calls.filter((c) => c.command === "harness_steer_queued");
  expect(sent.map((c) => [c.args.threadId, c.args.queueId])).toEqual([[7, "42"]]);
  expect(typeof sent[0].args.commandId).toBe("string");
});

test("a send carries the composer's mode to Rust; an idle one carries none", async () => {
  // `harnessSend`'s transport, with its invoke given: the module's own
  // captured Tauri's at import, which another test file may have done first.
  const { createHarnessMessaging } = await import("@/lib/harness/commands");
  const sent: unknown[][] = [];
  const messaging = createHarnessMessaging(async (command, args) => {
    sent.push([command, args.text, args.mode]);
    return 1 as never;
  }, async () => {});
  await messaging.send(7, "codex", "in metres", {}, "steer");
  await messaging.send(7, "codex", "later", {}, "queue");
  await messaging.send(7, "codex", "hello", {});
  expect(sent).toEqual([
    ["harness_send", "in metres", "steer"],
    ["harness_send", "later", "queue"],
    ["harness_send", "hello", undefined],
  ]);
});

test("with no thread open Send now sends nothing", async () => {
  calls.length = 0;
  await actionsFor(null).pending.steerQueued("42");
  expect(calls.filter((c) => c.command === "harness_steer_queued")).toEqual([]);
});
