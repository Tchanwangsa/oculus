/** Share concurrent reads, never settled results. A fresh read supersedes an
 *  older request so a write-triggered refresh cannot reuse pre-write rows. */
export function createPendingReader<K, T>(read: (key: K) => Promise<T>) {
  const pending = new Map<K, Promise<T>>();
  return (key: K, fresh = false): Promise<T> => {
    const existing = pending.get(key);
    if (!fresh && existing) return existing;
    const next = read(key).finally(() => {
      if (pending.get(key) === next) pending.delete(key);
    });
    pending.set(key, next);
    return next;
  };
}
