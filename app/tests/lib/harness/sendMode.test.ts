import { describe, expect, test } from "bun:test";
import {
  busyPlaceholder,
  DECLINE_PLACEHOLDER,
  deliveryMode,
  enterKey,
  isSlashCommand,
  modClick,
  RESTART_HINT,
  runControls,
  sendAction,
  STEER_HINT,
  steerKind,
  steerStatus,
  WAITING_PLACEHOLDER,
  type RunControlInputs,
  type SteerKind,
} from "@/lib/harness";
import { parseFollowUp } from "@/stores/chat/chatPrefsStore";

const native = { activeSteering: true, steerByRestart: false };
const restartOnly = { activeSteering: false, steerByRestart: true };
const neither = { activeSteering: false, steerByRestart: false };
const working = { running: true, liveRun: { stopping: false }, caps: native, questionWaits: false };

describe("steerKind: whether the running turn can take a message", () => {
  test("a bridge with active steering takes it natively; a restart-class one by restarting; others not", () => {
    expect(steerKind(working)).toBe("native");
    expect(steerKind({ ...working, caps: restartOnly })).toBe("restart");
    expect(steerKind({ ...working, caps: neither })).toBeNull();
    // Native wins where a bridge can do both.
    expect(steerKind({ ...working, caps: { activeSteering: true, steerByRestart: true } })).toBe("native");
  });

  test("nothing to steer: idle, a turn the CLI started itself, a run being stopped, a question waiting", () => {
    expect(steerKind({ ...working, running: false })).toBeNull();
    expect(steerKind({ ...working, liveRun: null })).toBeNull();
    expect(steerKind({ ...working, liveRun: { stopping: true } })).toBeNull();
    expect(steerKind({ ...working, questionWaits: true })).toBeNull();
  });
});

describe("sendAction: what a send does", () => {
  const base = { running: true, steer: "native" as SteerKind, followUp: "steer" as const, flip: false, command: false };

  test("the default follow-up is steer: Enter steers the running turn", () => {
    expect(parseFollowUp(null)).toBe("steer");
    expect(parseFollowUp("garbage")).toBe("steer");
    expect(parseFollowUp("queue")).toBe("queue");
    expect(sendAction(base)).toEqual({ kind: "steer", restart: false });
    expect(deliveryMode(sendAction(base))).toBe("steer");
  });

  test("⌘ flips it: ⌘↩ queues under steer, and steers under queue", () => {
    expect(sendAction({ ...base, flip: true })).toEqual({ kind: "queue" });
    expect(deliveryMode({ kind: "queue" })).toBe("queue");
    expect(sendAction({ ...base, followUp: "queue" })).toEqual({ kind: "queue" });
    expect(sendAction({ ...base, followUp: "queue", flip: true })).toEqual({ kind: "steer", restart: false });
  });

  test("a bridge without active steering never offers the steer path: it queues either way", () => {
    for (const flip of [false, true]) {
      expect(sendAction({ ...base, steer: null, flip })).toEqual({ kind: "queue" });
    }
  });

  test("a restart-class bridge steers by restarting, and the send asks Rust for a steer", () => {
    const a = sendAction({ ...base, steer: "restart" });
    expect(a).toEqual({ kind: "steer", restart: true });
    expect(deliveryMode(a)).toBe("steer");
  });

  test("a slash command always queues, whatever the setting or the flip", () => {
    expect(isSlashCommand("  /compact")).toBe(true);
    expect(isSlashCommand("a /path")).toBe(false);
    for (const flip of [false, true]) {
      for (const followUp of ["steer", "queue"] as const) {
        expect(sendAction({ ...base, command: true, flip, followUp })).toEqual({ kind: "queue" });
      }
    }
  });

  test("an idle chat sends normally and leaves Rust its default mode", () => {
    for (const flip of [false, true]) {
      const a = sendAction({ ...base, running: false, flip });
      expect(a).toEqual({ kind: "send" });
      expect(deliveryMode(a)).toBeUndefined();
    }
  });

  test("a waiting question makes nothing steerable, so a send queues (its box answers the question)", () => {
    const steer = steerKind({ ...working, questionWaits: true });
    expect(sendAction({ ...base, steer })).toEqual({ kind: "queue" });
  });
});

describe("enterKey", () => {
  const press = (patch: Partial<{ shiftKey: boolean; metaKey: boolean; ctrlKey: boolean }> = {}, key = "Enter") => ({
    key, shiftKey: false, metaKey: false, ctrlKey: false, ...patch,
  });

  test("Enter sends, ⌘↩ flips, ⌘⇧↩ steers the oldest waiting message, Shift+Enter breaks the line", () => {
    expect(enterKey(press(), false, true)).toBe("send");
    expect(enterKey(press({ metaKey: true }), false, true)).toBe("flip");
    expect(enterKey(press({ metaKey: true, shiftKey: true }), false, true)).toBe("steer-oldest");
    expect(enterKey(press({ shiftKey: true }), false, true)).toBeNull();
    expect(enterKey(press({}, "a"), false, true)).toBeNull();
  });

  test("off macOS the modifier is Ctrl, and ⌘ means nothing", () => {
    expect(enterKey(press({ ctrlKey: true }), false, false)).toBe("flip");
    expect(enterKey(press({ ctrlKey: true, shiftKey: true }), false, false)).toBe("steer-oldest");
    expect(enterKey(press({ metaKey: true }), false, false)).toBe("send");
    expect(modClick({ metaKey: false, ctrlKey: true }, false)).toBe(true);
    expect(modClick({ metaKey: true, ctrlKey: false }, true)).toBe(true);
    expect(modClick(undefined, true)).toBe(false);
  });

  test("an unscoped box holds Enter and ⌘↩ alike", () => {
    expect(enterKey(press(), true, true)).toBe("hold");
    expect(enterKey(press({ metaKey: true }), true, true)).toBe("hold");
  });
});

describe("the send button shows its action before the press", () => {
  const running: RunControlInputs = {
    running: true, liveRun: { id: 3, stopping: false }, stopSent: false, held: false, hasDraft: true, ready: true, writing: false,
  };

  test("steer: Steer message, its tooltip naming the current turn", () => {
    expect(runControls({ ...running, action: { kind: "steer", restart: false } }).send).toEqual({
      kind: "steer", label: "Steer message", hint: STEER_HINT, disabled: false,
    });
  });

  test("restart-class: the tooltip says it stops and restarts the turn", () => {
    expect(runControls({ ...running, action: { kind: "steer", restart: true } }).send?.hint).toBe(RESTART_HINT);
    expect(RESTART_HINT).toBe("Stops and restarts the turn");
  });

  test("queue: Queue message; and with no action given a running chat still queues", () => {
    expect(runControls({ ...running, action: { kind: "queue" } }).send?.label).toBe("Queue message");
    expect(runControls(running).send?.kind).toBe("queue");
  });

  test("an idle chat's button sends whatever action is passed", () => {
    expect(runControls({ ...running, running: false, liveRun: null, action: { kind: "steer", restart: false } }).send?.label).toBe("Send message");
  });
});

describe("busyPlaceholder", () => {
  const steer = { kind: "steer", restart: false } as const;
  const queue = { kind: "queue" } as const;

  test("an approval waiting and a message that would steer: send to decline and redirect", () => {
    expect(busyPlaceholder({ approval: true, plain: steer, steer: "native" })).toBe(DECLINE_PLACEHOLDER);
    expect(DECLINE_PLACEHOLDER).toBe("Send a message to decline and redirect the agent");
    // The dock says it too: it is what a send does.
    expect(busyPlaceholder({ approval: true, plain: steer, steer: "native", compact: true })).toBe(DECLINE_PLACEHOLDER);
  });

  test("an approval waiting on a bridge that cannot steer: the message waits its turn", () => {
    expect(busyPlaceholder({ approval: true, plain: queue, steer: null })).toBe(WAITING_PLACEHOLDER);
  });

  test("working: the keys are named for what each does", () => {
    expect(busyPlaceholder({ approval: false, plain: steer, steer: "native", mac: true })).toBe("Working… ↵ to steer, ⌘↵ to queue");
    expect(busyPlaceholder({ approval: false, plain: queue, steer: "native", mac: true })).toBe("Working… ↵ to queue, ⌘↵ to steer");
    expect(busyPlaceholder({ approval: false, plain: steer, steer: "restart", mac: false })).toBe("Working… Enter to restart with it, Ctrl+Enter to queue");
    expect(busyPlaceholder({ approval: false, plain: queue, steer: null })).toBe("Working… your next message waits its turn");
    expect(busyPlaceholder({ approval: false, plain: steer, steer: "native", compact: true })).toBe("Working…");
  });
});

describe("steerStatus: a steer row's delivery in words", () => {
  test("sending and written say it is on its way; delivered says nothing", () => {
    expect(steerStatus("sending")).toMatchObject({ tone: "muted", busy: true });
    expect(steerStatus("written")).toEqual(steerStatus("sending"));
    expect(steerStatus("delivered")).toBeNull();
  });

  test("unknown warns it may not have arrived and is never resent", () => {
    expect(steerStatus("unknown")).toEqual({
      text: "This may not have reached the agent. It won't be sent again.", tone: "warning", busy: false,
    });
  });

  test("restart waits on the turn to stop", () => {
    expect(steerStatus("restart")?.text).toContain("restart");
  });
});
