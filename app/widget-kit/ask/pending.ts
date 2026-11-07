/**
 * The frame's half of `oculus/ask`'s requests: each gets an id made here,
 * is posted to the parent, and waits in `waiting` until a `reply` with that
 * id comes back (the runtime hands replies to `settle`). A reply for an id
 * not waiting is ignored. Shared by the `ask` entry and the runtime through
 * one chunk, so there is one map per frame.
 */
import type { ReplyMessage } from "../protocol";
import { post } from "../runtime/post";

/** A request as the module makes it; `request` adds the id. */
export type Outgoing = { kind: "ask"; prompt: string; schema: Record<string, unknown> } | { kind: "send"; text: string };

const waiting = new Map<string, { resolve: (value: unknown) => void; reject: (error: Error) => void }>();
let made = 0;

/** Posts `body` and answers with the parent's reply: its value, or an
 *  `Error` carrying the reason. */
export function request(body: Outgoing, send: (message: Outgoing & { id: string }) => void = post): Promise<unknown> {
  const id = `r${++made}`;
  return new Promise((resolve, reject) => {
    waiting.set(id, { resolve, reject });
    send({ ...body, id });
  });
}

/** Resolves or rejects the request `reply` answers; false when none waits. */
export function settle(reply: ReplyMessage): boolean {
  const w = waiting.get(reply.id);
  if (!w) return false;
  waiting.delete(reply.id);
  if (reply.ok) w.resolve(reply.value);
  else w.reject(new Error(reply.error));
  return true;
}

/** How many requests wait; for tests. */
export const waitingCount = () => waiting.size;
