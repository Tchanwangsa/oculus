/** Edit, rewind and retry truncate the thread, so they are absent for a
 *  provider that cannot take a question back out of its own context
 *  (the bridge's `rewind` capability). */
export interface QuestionActions {
  /** Ask it again, differently. The thread rewinds to this row. */
  edit?: (itemId: number, text: string) => void;
  /** Take the thread back to just before this question and hand its words to
   *  the composer. */
  rewind?: (itemId: number) => void;
  /** Ask the same question again, unchanged. */
  retry?: (itemId: number, text: string) => void;
  /** Send a new message on the open thread — what an approved permission
   *  carries on with. */
  followUp: (text: string) => Promise<void>;
}

/** The waiting messages' commands. Each rejects with Rust's reason, which
 *  the list shows beside the messages. */
export interface PendingActions {
  editQueued: (queueId: string, text: string) => Promise<void>;
  unqueue: (queueId: string) => Promise<void>;
  /** Every waiting message's id, first to go out first. */
  reorder: (order: string[]) => Promise<void>;
  /** "Send now": the message into the running turn (`harnessSteerQueued`). */
  steerQueued: (queueId: string) => Promise<void>;
}
