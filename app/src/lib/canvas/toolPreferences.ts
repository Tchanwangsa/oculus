/** Device preferences stay local; collaborators share drawing content only. */
export type EraserMode = 'stroke' | 'partial';
export interface CanvasToolPreferences {
  eraserAutoReturn: boolean;
  /** `stroke` removes whatever the eraser touches whole; `partial` rubs out the part of a freehand stroke under it. */
  eraserMode: EraserMode;
  /** The partial eraser's width in screen pixels. */
  eraserSize: number;
}

const STORAGE_KEY = 'oculus-canvas-tool-preferences-v1';
export const ERASER_SIZES = { min: 4, max: 80 } as const;
const defaults: CanvasToolPreferences = { eraserAutoReturn: false, eraserMode: 'stroke', eraserSize: 20 };

export function loadCanvasToolPreferences(): CanvasToolPreferences {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null');
    if (!saved || typeof saved !== 'object') return { ...defaults };
    const { eraserAutoReturn, eraserMode, eraserSize } = saved as Record<string, unknown>;
    return {
      eraserAutoReturn: eraserAutoReturn === true,
      eraserMode: eraserMode === 'partial' ? 'partial' : 'stroke',
      eraserSize: typeof eraserSize === 'number' && Number.isFinite(eraserSize) ? Math.min(ERASER_SIZES.max, Math.max(ERASER_SIZES.min, eraserSize)) : defaults.eraserSize,
    };
  } catch { return { ...defaults }; }
}

export function saveCanvasToolPreferences(preferences: CanvasToolPreferences): void {
  // Browsing modes that deny storage can still use the option for this session.
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(preferences)); } catch { /* storage unavailable */ }
}
