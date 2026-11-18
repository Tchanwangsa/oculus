import type { WhiteboardPen } from './model';
import { MARKERS, PENS } from './pens';
import type { StylePatch } from './style';
import { adoptLegacyKey } from './storageKeys';

/**
 * The pens and highlighters a device keeps ready: each remembers its pen, colour, width and opacity, and the one in
 * use is what the Pen or Highlighter tool draws with. Changing the look while a tool is chosen changes its preset in
 * use. Device-local, like the tool preferences; collaborators share drawing content only.
 */
export interface InkPreset { pen: WhiteboardPen; stroke: string; strokeWidth: number; opacity: number }
export interface InkPresets { pens: InkPreset[]; pen: number; markers: InkPreset[]; marker: number }
export type InkTool = 'freedraw' | 'highlighter';

const STORAGE_KEY = 'oculus-whiteboard-ink-presets-v1';
const ball = (stroke: string): InkPreset => ({ pen: 'ball', stroke, strokeWidth: 2, opacity: 1 });
const marker = (stroke: string): InkPreset => ({ pen: 'roundMarker', stroke, strokeWidth: 16, opacity: 0.4 });
export const DEFAULT_INK_PRESETS: InkPresets = {
  pens: [ball('#292b32'), ball('#3b82b6'), ball('#c8483f'), { pen: 'fountain', stroke: '#292b32', strokeWidth: 4, opacity: 1 }], pen: 0,
  markers: [marker('#ffe14d'), marker('#8be28f'), marker('#ff8fc7')], marker: 0,
};

const finite = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value);
/** A saved list is used only if it has as many presets as the default and each is whole and of the list's kind. */
function presets(value: unknown, fallback: InkPreset[], kinds: readonly WhiteboardPen[]): InkPreset[] {
  if (!Array.isArray(value) || value.length !== fallback.length) return fallback;
  const read = value.map((item: Partial<InkPreset> | null) => item && kinds.includes(item.pen as WhiteboardPen) && typeof item.stroke === 'string'
    && finite(item.strokeWidth) && item.strokeWidth > 0 && finite(item.opacity) && item.opacity >= 0 && item.opacity <= 1
    ? { pen: item.pen as WhiteboardPen, stroke: item.stroke, strokeWidth: item.strokeWidth, opacity: item.opacity } : null);
  return read.every((item): item is InkPreset => item !== null) ? read : fallback;
}
const index = (value: unknown, length: number) => Number.isInteger(value) && (value as number) >= 0 && (value as number) < length ? value as number : 0;

export function loadInkPresets(): InkPresets {
  adoptLegacyKey(STORAGE_KEY);
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null') as Partial<InkPresets> | null;
    if (!saved || typeof saved !== 'object') return DEFAULT_INK_PRESETS;
    const pens = presets(saved.pens, DEFAULT_INK_PRESETS.pens, PENS), markers = presets(saved.markers, DEFAULT_INK_PRESETS.markers, MARKERS);
    return { pens, pen: index(saved.pen, pens.length), markers, marker: index(saved.marker, markers.length) };
  } catch { return DEFAULT_INK_PRESETS; }
}

export function saveInkPresets(value: InkPresets): void {
  adoptLegacyKey(STORAGE_KEY);
  // Browsing modes that deny storage can still use the presets for this session.
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(value)); } catch { /* storage unavailable */ }
}

const list = (tool: InkTool) => tool === 'highlighter' ? 'markers' as const : 'pens' as const;
const cursor = (tool: InkTool) => tool === 'highlighter' ? 'marker' as const : 'pen' as const;
export const toolPresets = (value: InkPresets, tool: InkTool) => ({ items: value[list(tool)], active: value[cursor(tool)] });
export const activePreset = (value: InkPresets, tool: InkTool): InkPreset => value[list(tool)][value[cursor(tool)]];

export function selectPreset(value: InkPresets, tool: InkTool, at: number): InkPresets {
  return at === value[cursor(tool)] || !value[list(tool)][at] ? value : { ...value, [cursor(tool)]: at };
}

/** The tool's preset in use with the patch's ink fields; a pen of the other tool's kind is ignored. */
export function patchPreset(value: InkPresets, tool: InkTool, patch: StylePatch): InkPresets {
  const current = activePreset(value, tool), next = { ...current };
  if (patch.pen !== undefined && (tool === 'highlighter' ? MARKERS : PENS).includes(patch.pen)) next.pen = patch.pen;
  if (patch.stroke !== undefined) next.stroke = patch.stroke;
  if (patch.strokeWidth !== undefined) next.strokeWidth = patch.strokeWidth;
  if (patch.opacity !== undefined) next.opacity = patch.opacity;
  if (next.pen === current.pen && next.stroke === current.stroke && next.strokeWidth === current.strokeWidth && next.opacity === current.opacity) return value;
  return { ...value, [list(tool)]: value[list(tool)].map((item, i) => i === value[cursor(tool)] ? next : item) };
}
