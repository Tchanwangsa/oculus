/**
 * What a reload or reopen resumes, kept on this device only: the tool and drawing style last used, and each drawing's
 * camera. Collaborators never see it, and a drawing's file never holds it.
 */
import type { CanvasTool, CanvasViewport } from './model';
import { DEFAULT_DRAWING_STYLE, type DrawingStyle } from './style';

const TOOL_KEY = 'oculus-canvas-tool-state-v1';
const CAMERA_KEY = 'oculus-canvas-cameras-v1';
/** The most recently viewed drawings whose camera is kept; older ones are forgotten. */
const CAMERA_LIMIT = 60;

/** Crop needs an image chosen in this session, so it resumes as Select. */
const RESUMABLE: ReadonlySet<CanvasTool> = new Set(['select', 'capture', 'hand', 'rectangle', 'ellipse', 'diamond', 'arrow', 'line', 'freedraw', 'text', 'eraser']);

const read = (key: string): unknown => { try { return JSON.parse(localStorage.getItem(key) ?? 'null'); } catch { return null; } };
const write = (key: string, value: unknown) => { try { localStorage.setItem(key, JSON.stringify(value)); } catch { /* storage unavailable */ } };
const isRecord = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);

export interface ToolState { tool: CanvasTool; style: DrawingStyle }

/** Each saved style field is kept only when it has the default's type, so an old or damaged entry falls back field by field. */
export function loadToolState(): ToolState {
  const saved = read(TOOL_KEY);
  const tool = isRecord(saved) && typeof saved.tool === 'string' && RESUMABLE.has(saved.tool as CanvasTool) ? saved.tool as CanvasTool : 'select';
  const style: Record<string, unknown> = { ...DEFAULT_DRAWING_STYLE };
  if (isRecord(saved) && isRecord(saved.style)) {
    for (const [key, fallback] of Object.entries(DEFAULT_DRAWING_STYLE)) {
      const value = saved.style[key];
      if (typeof value === typeof fallback && (typeof value !== 'number' || Number.isFinite(value))) style[key] = value;
    }
  }
  return { tool, style: style as unknown as DrawingStyle };
}

export function saveToolState(state: ToolState): void { write(TOOL_KEY, state); }

type Cameras = Record<string, CanvasViewport & { at: number }>;
const cameras = (): Cameras => { const saved = read(CAMERA_KEY); return isRecord(saved) ? saved as Cameras : {}; };
const validCamera = (value: unknown): value is CanvasViewport =>
  isRecord(value) && [value.x, value.y, value.zoom].every(part => typeof part === 'number' && Number.isFinite(part)) && (value.zoom as number) > 0;

export function loadCamera(key: string): CanvasViewport | null {
  const saved = cameras()[key];
  return validCamera(saved) ? { x: saved.x, y: saved.y, zoom: saved.zoom } : null;
}

export function saveCamera(key: string, { x, y, zoom }: CanvasViewport, now = Date.now()): void {
  const all = cameras();
  all[key] = { x, y, zoom, at: now };
  const keys = Object.keys(all);
  if (keys.length > CAMERA_LIMIT) {
    keys.sort((a, b) => (all[a].at ?? 0) - (all[b].at ?? 0));
    for (const old of keys.slice(0, keys.length - CAMERA_LIMIT)) delete all[old];
  }
  write(CAMERA_KEY, all);
}
