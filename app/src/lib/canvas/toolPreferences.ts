/** Device preferences stay local; collaborators share drawing content only. */
export interface CanvasToolPreferences { eraserAutoReturn: boolean }

const STORAGE_KEY = 'oculus-canvas-tool-preferences-v1';
const defaults: CanvasToolPreferences = { eraserAutoReturn: false };

export function loadCanvasToolPreferences(): CanvasToolPreferences {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null');
    return { eraserAutoReturn: !!saved && typeof saved === 'object' && 'eraserAutoReturn' in saved && saved.eraserAutoReturn === true };
  } catch { return { ...defaults }; }
}

export function saveCanvasToolPreferences(preferences: CanvasToolPreferences): void {
  // Browsing modes that deny storage can still use the option for this session.
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(preferences)); } catch { /* storage unavailable */ }
}
