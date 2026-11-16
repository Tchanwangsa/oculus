import { shortcut } from "@/lib/platform";
import type { Capabilities } from "./capabilities";
import type { DeliveryMode } from "./commands";
import type { HarnessRun, SteerDelivery } from "./types";

/** What Enter does while the agent works (Settings → Chat): write the message
 *  into the running turn, or wait for it to end. ⌘↩ does the other. */
export type FollowUp = "steer" | "queue";

export const DEFAULT_FOLLOW_UP: FollowUp = "steer";

/** How the running turn can take a message: written into it (`native`),
 *  by stopping it and starting again with the message (`restart`), or not
 *  at all (null: the message queues). */
export type SteerKind = "native" | "restart" | null;

export interface SteerInputs {
  /** A turn is open on the chat. */
  running: boolean;
  /** The run that turn belongs to; null for a turn the CLI started itself,
   *  which has no run to join. */
  liveRun: Pick<HarnessRun, "stopping"> | null;
  /** The bridge of the running run, never the composer's pick. */
  caps: Pick<Capabilities, "activeSteering" | "steerByRestart">;
  /** A question waits: typed text is its answer, so nothing steers. */
  questionWaits: boolean;
}

/**
 * Whether a message can go into the running turn, and how — mirroring
 * `resolve_delivery` (`harness/manager/policy.rs`) for the cases the webview
 * can see. A run its provider has not taken yet looks running here; Rust
 * queues a steer into it.
 */
export function steerKind(i: SteerInputs): SteerKind {
  if (!i.running || !i.liveRun || i.liveRun.stopping || i.questionWaits) return null;
  if (i.caps.activeSteering) return "native";
  return i.caps.steerByRestart ? "restart" : null;
}

/** Rust's test for a command, which never goes into a turn. */
export const isSlashCommand = (text: string) => text.trimStart().startsWith("/");

/** What a send will do: start a run on an idle chat, wait behind the running
 *  turn, or steer it (`restart`: by stopping and starting it again). */
export type SendAction = { kind: "send" } | { kind: "queue" } | { kind: "steer"; restart: boolean };

/**
 * The send's action. Idle chats send. Into a running turn the chat's
 * follow-up behaviour decides, `flip` (⌘ held, ⌘↩) inverting it; a turn
 * that cannot be steered, and a slash command, always queue.
 */
export function sendAction(i: {
  running: boolean;
  steer: SteerKind;
  followUp: FollowUp;
  flip: boolean;
  command: boolean;
}): SendAction {
  if (!i.running) return { kind: "send" };
  const wants = (i.followUp === "steer") !== i.flip;
  if (!wants || !i.steer || i.command) return { kind: "queue" };
  return { kind: "steer", restart: i.steer === "restart" };
}

/** The mode `harnessSend` carries. An idle send leaves Rust its default;
 *  a steer asks for `steer`, which Rust turns into a restart where the bridge
 *  can only restart. */
export function deliveryMode(a: SendAction): DeliveryMode | undefined {
  return a.kind === "queue" ? "queue" : a.kind === "steer" ? "steer" : undefined;
}

export const STEER_LABEL = "Steer message";
export const QUEUE_LABEL = "Queue message";
export const SEND_NOW = "Send now";
/** "Send now"'s tooltip, and a steer's, on a bridge that takes the message
 *  into the turn. */
export const STEER_HINT = "Send into the current turn";
/** The same, on a bridge that can only stop and start again. */
export const RESTART_HINT = "Stops and restarts the turn";

/** The tooltip that says what a steer will do. */
export const steerHint = (kind: Exclude<SteerKind, null>) => (kind === "restart" ? RESTART_HINT : STEER_HINT);

/** The box's hint while an approval waits and a message would steer: Rust
 *  declines the request and writes the message into the turn. */
export const DECLINE_PLACEHOLDER = "Send a message to decline and redirect the agent";

/** The box's hint while an approval waits and a message would queue behind
 *  the request. */
export const WAITING_PLACEHOLDER = "Waiting for your approval… a message waits its turn";

/**
 * The empty box's hint while the agent works or an approval waits. `plain`
 * is what Enter would do (`sendAction` unflipped, no command); the keys are
 * named where ⌘↩ does something else. `compact` (the lecture dock) says
 * only that it works.
 */
export function busyPlaceholder(i: { approval: boolean; plain: SendAction; steer: SteerKind; compact?: boolean; mac?: boolean }): string {
  if (i.approval) return i.plain.kind === "steer" ? DECLINE_PLACEHOLDER : WAITING_PLACEHOLDER;
  if (i.compact) return "Working…";
  if (!i.steer) return "Working… your next message waits its turn";
  const enter = shortcut("enter", i.mac);
  const flip = shortcut("mod+enter", i.mac);
  const into = i.steer === "restart" ? "restart with it" : "steer";
  return i.plain.kind === "steer" ? `Working… ${enter} to ${into}, ${flip} to queue` : `Working… ${enter} to queue, ${flip} to ${into}`;
}

/** Whether a click on the send button carried ⌘ (Ctrl off macOS). */
export const modClick = (e: { metaKey: boolean; ctrlKey: boolean } | undefined, mac: boolean) =>
  !!e && (mac ? e.metaKey : e.ctrlKey);

/**
 * What ⌘↩, ⌘⇧↩ and Enter mean in the box: `send` (the follow-up behaviour),
 * `flip` (its opposite), `steer-oldest` (the oldest waiting message into the
 * turn), `hold` (no scope yet: nothing, not even a line break) or the
 * editor's own (null: Shift+Enter, every other key). `mod` is ⌘ on macOS and
 * Ctrl elsewhere.
 */
export function enterKey(
  e: { key: string; shiftKey: boolean; metaKey: boolean; ctrlKey: boolean },
  unscoped: boolean,
  mac: boolean,
): "send" | "flip" | "steer-oldest" | "hold" | null {
  if (e.key !== "Enter") return null;
  const mod = mac ? e.metaKey : e.ctrlKey;
  if (mod && e.shiftKey) return "steer-oldest";
  if (e.shiftKey) return null;
  if (unscoped) return "hold";
  return mod ? "flip" : "send";
}

/**
 * What a steer row says under its bubble, by its delivery (`store/steer.rs`):
 * nothing once the provider echoed it, "Sending…" until then, a warning
 * that it may not have arrived after a broken write (it is never resent),
 * and the restart it waits on. `busy` draws a spinner.
 */
export function steerStatus(delivery: SteerDelivery): { text: string; tone: "muted" | "warning"; busy: boolean } | null {
  switch (delivery) {
    case "delivered":
      return null;
    case "sending":
    case "written":
      return { text: "Sending into the turn…", tone: "muted", busy: true };
    case "restart":
      return { text: "Stopping the turn to restart with this…", tone: "muted", busy: true };
    case "unknown":
      return { text: "This may not have reached the agent. It won't be sent again.", tone: "warning", busy: false };
  }
}
