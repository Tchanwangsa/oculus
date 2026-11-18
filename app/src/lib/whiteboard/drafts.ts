import { invoke } from '@tauri-apps/api/core';
import { adoptLegacyKey } from './storageKeys';

const fileId = (key: string) => {
  const match = /^oculus:whiteboard-draft:([1-9]\d*)$/.exec(key);
  const id = match ? Number(match[1]) : NaN;
  if (!Number.isSafeInteger(id) || id < 1) throw new Error('Invalid whiteboard draft key');
  return id;
};
const removeLegacy = (key: string) => { try { localStorage.removeItem(key); } catch { /* The native recovery record owns this draft. */ } };

/** Desktop drafts live on disk; existing localStorage recovery is preserved until migrated. */
export const whiteboardDrafts = {
  async getItem(key: string): Promise<string | null> {
    const id = fileId(key);
    adoptLegacyKey(key);
    const value = await invoke<string | null>('read_whiteboard_draft', { fileId: id });
    if (value !== null) return value;
    let previous: string | null = null;
    try { previous = localStorage.getItem(key); } catch { /* no readable legacy draft */ }
    if (previous !== null) {
      try {
        await invoke('write_whiteboard_draft', { fileId: id, content: previous });
        removeLegacy(key);
      } catch { /* Keep the readable legacy draft if migration fails. */ }
    }
    return previous;
  },
  async setItem(key: string, content: string): Promise<void> {
    await invoke('write_whiteboard_draft', { fileId: fileId(key), content });
    removeLegacy(key);
  },
  async removeItem(key: string): Promise<void> {
    await invoke('delete_whiteboard_draft', { fileId: fileId(key) });
    removeLegacy(key);
  },
};
