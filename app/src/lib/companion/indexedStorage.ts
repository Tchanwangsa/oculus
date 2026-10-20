import type { DraftStorage, StorageLike } from './env';

/** IndexedDB owns large recovery records; localStorage is read only for migration. */
export function indexedDraftStorage(factory: IDBFactory, legacy: StorageLike | null): DraftStorage {
  let database: Promise<IDBDatabase> | undefined;
  const open = () => database ??= new Promise<IDBDatabase>((resolve, reject) => {
    const request = factory.open('oculus-companion-recovery', 1);
    request.onupgradeneeded = () => request.result.createObjectStore('drafts');
    request.onerror = () => reject(request.error);
    request.onblocked = () => reject(new Error('Recovery storage is waiting for another tab to close.'));
    request.onsuccess = () => {
      const db = request.result;
      db.onversionchange = () => { db.close(); database = undefined; };
      resolve(db);
    };
  }).catch(error => { database = undefined; throw error; });

  const transaction = async <T>(mode: IDBTransactionMode, action: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> => {
    const db = await open();
    return new Promise<T>((resolve, reject) => {
      const tx = db.transaction('drafts', mode);
      const request = action(tx.objectStore('drafts'));
      tx.oncomplete = () => resolve(request.result);
      tx.onerror = () => reject(tx.error ?? request.error);
      tx.onabort = () => reject(tx.error ?? new Error('Recovery storage was interrupted.'));
    });
  };
  const removeLegacy = (key: string) => { try { legacy?.removeItem(key); } catch { /* IndexedDB owns the record. */ } };
  return {
    async getItem(key) {
      const value = await transaction('readonly', store => store.get(key));
      if (typeof value === 'string') return value;
      let previous: string | null = null;
      try { previous = legacy?.getItem(key) ?? null; } catch { /* no readable legacy draft */ }
      if (previous !== null) {
        await transaction('readwrite', store => store.put(previous, key));
        removeLegacy(key);
      }
      return previous;
    },
    async setItem(key, value) {
      await transaction('readwrite', store => store.put(value, key));
      removeLegacy(key);
    },
    async removeItem(key) {
      await transaction('readwrite', store => store.delete(key));
      removeLegacy(key);
    },
  };
}
