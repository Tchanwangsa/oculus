/** A file is visible to readers only after its metadata write has committed. */
export const SCRAPED_FILE_SAVED_EVENT = "oculus:scraped-file-saved";
export const SCRAPED_FILE_FAILED_EVENT = "oculus:scraped-file-failed";

/** Backend event callbacks are not awaited by Tauri. Serialize their writes
 *  and keep failures until the run finishes, so a completion cannot overtake
 *  the last file insert or turn a failed insert into a successful ledger row. */
export class SyncWriteQueue {
  private tail: Promise<void> = Promise.resolve();
  private failures = new Map<number | null, { count: number; first: string }>();

  enqueue(
    runId: number | null,
    label: string,
    write: () => Promise<void>,
    onError: (message: string) => void,
  ): Promise<boolean> {
    const result = this.tail.then(write).then(() => true, (reason) => {
      const message = `${label}: ${String(reason)}`;
      const failure = this.failures.get(runId);
      this.failures.set(runId, { count: (failure?.count ?? 0) + 1, first: failure?.first ?? message });
      onError(message);
      return false;
    });
    this.tail = result.then(() => {});
    return result;
  }

  async drain(): Promise<void> {
    // Include work appended while an earlier file is still committing.
    for (;;) {
      const pending = this.tail;
      await pending;
      if (pending === this.tail) return;
    }
  }

  takeFailure(runId: number | null): string | null {
    const failure = this.failures.get(runId);
    this.failures.delete(runId);
    return failure
      ? `Could not save ${failure.count} library update${failure.count === 1 ? "" : "s"}. ${failure.first}`
      : null;
  }
}
