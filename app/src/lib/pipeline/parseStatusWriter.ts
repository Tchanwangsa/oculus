/** Serializes status writes per file while coalescing progress heartbeats.
 * Only an update that found its row counts as persisted. Scrape mutations
 * share the queue so a new parse cannot race its row insertion or reset. */
export function createParseStatusWriter(
  persist: (subjectId: number, path: string, status: string) => Promise<boolean>,
) {
  type Entry = { status: string | null; tail: Promise<void> };
  const entries = new Map<string, Entry>();

  function enqueue(
    subjectId: number,
    path: string,
    action: (entry: Entry) => Promise<void>,
    release: boolean,
  ): Promise<void> {
    const key = `${subjectId}\0${path}`;
    let entry = entries.get(key);
    if (!entry) {
      entry = { status: null, tail: Promise.resolve() };
      entries.set(key, entry);
    }
    const current = entry;
    const pending = current.tail.then(() => action(current));
    const tail = pending.catch(() => {}).finally(() => {
      if (current.tail === tail && (release || current.status == null)) entries.delete(key);
    });
    current.tail = tail;
    return pending;
  }

  return {
    write(subjectId: number, path: string, status: string): Promise<void> {
      return enqueue(subjectId, path, async (entry) => {
        if (entry.status === status) return;
        entry.status = null;
        if (await persist(subjectId, path, status)) entry.status = status;
      }, status === "quality" || status === "error" || status === "skipped");
    },
    mutate(subjectId: number, path: string, action: () => Promise<void>): Promise<void> {
      return enqueue(subjectId, path, async (entry) => {
        entry.status = null;
        await action();
      }, true);
    },
  };
}
