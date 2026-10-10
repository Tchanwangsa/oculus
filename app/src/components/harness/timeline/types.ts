/** Edit, rewind and retry truncate the thread, so they are absent for a
 *  provider that cannot take a question back out of its own context
 *  (`ProviderInfo.rewind`). */
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

export interface PendingActions {
  editQueued: (queueId: string, text: string) => void;
  unqueue: (queueId: string) => void;
}
