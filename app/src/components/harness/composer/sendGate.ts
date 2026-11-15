/** The placeholder a new chat's box shows until the headline has a subject. */
export const UNSCOPED_PLACEHOLDER = "Choose a subject above to start a chat";

/**
 * Whether the send button is live: there is something to send and the
 * thread's scope is chosen. Words, pictures and pasted cards still collect
 * while it is not, so nothing is lost while the student picks.
 */
export function canSend({ ready, unscoped }: { ready: boolean; unscoped: boolean }): boolean {
  return ready && !unscoped;
}

/**
 * What a key in the box does to sending: Enter sends, or with no scope yet is
 * swallowed (`hold`: no send, and no line break either); Shift+Enter, and
 * every other key, is the editor's (`null`).
 */
export function enterAction(
  e: { key: string; shiftKey: boolean },
  unscoped: boolean,
): "send" | "hold" | null {
  if (e.key !== "Enter" || e.shiftKey) return null;
  return unscoped ? "hold" : "send";
}

/** The box's hint while a request waits for an answer above it: the box
 *  stays usable, and a send queues behind the request. */
export const WAITING_PLACEHOLDER = "Waiting for your approval… a message waits its turn";
