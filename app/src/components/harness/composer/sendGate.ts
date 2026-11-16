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
