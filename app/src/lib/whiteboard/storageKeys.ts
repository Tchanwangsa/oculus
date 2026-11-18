const adopted = new Set<string>();

/**
 * Moves an entry this device still holds under the key's `canvas` name (`oculus-canvas-…`,
 * `oculus:canvas-draft:…`) to `key`, once per key and session, so the tool, cameras, presets and
 * drafts it remembers carry over. An entry already under `key` wins and the old one is left alone.
 */
export function adoptLegacyKey(key: string): void {
  if (adopted.has(key)) return;
  adopted.add(key);
  const legacy = key.replace('whiteboard', 'canvas');
  try {
    if (legacy === key || localStorage.getItem(key) !== null) return;
    const value = localStorage.getItem(legacy);
    if (value === null) return;
    localStorage.setItem(key, value);
    localStorage.removeItem(legacy);
  } catch { /* storage unavailable */ }
}
