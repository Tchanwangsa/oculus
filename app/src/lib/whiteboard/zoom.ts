export interface ZoomLimits { min: number; max: number }

/** A whiteboard's zoom range; a notebook narrows the bottom of it to its pages (`zoomLimits` in `paper.ts`). */
export const CANVAS_ZOOM_LIMITS: ZoomLimits = { min: 0.1, max: 8 };

export const clampZoom = (zoom: number, limits: ZoomLimits = CANVAS_ZOOM_LIMITS) => Math.max(limits.min, Math.min(limits.max, zoom));
