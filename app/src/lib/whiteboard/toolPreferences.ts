import { ERASER_SIZES, type EraserMode, type EraserSize } from './erase';

/** Device preferences stay local; collaborators share drawing content only. */
export interface CanvasToolPreferences {
  /** Lifting the eraser after a completed erase returns to the tool chosen before it. */
  eraserAutoReturn: boolean;
  /** `stroke` erases whole elements; `partial` cuts strokes where the eraser passes (`erase.ts`). */
  eraserMode: EraserMode;
  eraserSize: EraserSize;
}

const STORAGE_KEY = 'oculus-canvas-tool-preferences-v1';
export const DEFAULT_TOOL_PREFERENCES: CanvasToolPreferences = { eraserAutoReturn: false, eraserMode: 'stroke', eraserSize: 'medium' };

/** Each field is read on its own, so a value saved before a field existed, or a corrupt one, falls back to its default. */
export function loadCanvasToolPreferences(): CanvasToolPreferences {
  let saved: Record<string, unknown> = {};
  try {
    const value: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null');
    if (value && typeof value === 'object' && !Array.isArray(value)) saved = value as Record<string, unknown>;
  } catch { /* unreadable: defaults */ }
  return {
    eraserAutoReturn: saved.eraserAutoReturn === true,
    eraserMode: saved.eraserMode === 'partial' ? 'partial' : DEFAULT_TOOL_PREFERENCES.eraserMode,
    eraserSize: typeof saved.eraserSize === 'string' && Object.prototype.hasOwnProperty.call(ERASER_SIZES, saved.eraserSize) ? saved.eraserSize as EraserSize : DEFAULT_TOOL_PREFERENCES.eraserSize,
  };
}

export function saveCanvasToolPreferences(preferences: CanvasToolPreferences): void {
  // Browsing modes that deny storage can still use the option for this session.
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(preferences)); } catch { /* storage unavailable */ }
}
