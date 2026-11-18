import { invoke } from '@tauri-apps/api/core';

const fileId = (key: string) => {
  const match = /^oculus:canvas-draft:([1-9]\d*)$/.exec(key);
  const id = match ? Number(match[1]) : NaN;
  if (!Number.isSafeInteger(id) || id < 1) throw new Error('Invalid canvas draft key');
  return id;
};
const removeLegacy = (key: string) => { try { localStorage.removeItem(key); } catch { /* The native recovery record owns this draft. */ } };

/** Desktop drafts live on disk; existing localStorage recovery is preserved until migrated. */
export const canvasDrafts = {
  async getItem(key: string): Promise<string | null> {
    const id = fileId(key);
    const value = await invoke<string | null>('read_canvas_draft', { fileId: id });
    if (value !== null) return value;
    let previous: string | null = null;
    try { previous = localStorage.getItem(key); } catch { /* no readable legacy draft */ }
    if (previous !== null) {
      try {
        await invoke('write_canvas_draft', { fileId: id, content: previous });
        removeLegacy(key);
      } catch { /* Keep the readable legacy draft if migration fails. */ }
    }
    return previous;
  },
  async setItem(key: string, content: string): Promise<void> {
    await invoke('write_canvas_draft', { fileId: fileId(key), content });
    removeLegacy(key);
  },
  async removeItem(key: string): Promise<void> {
    await invoke('delete_canvas_draft', { fileId: fileId(key) });
    removeLegacy(key);
  },
};
