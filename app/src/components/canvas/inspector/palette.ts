import type { PaletteColor } from '@/components/ui/color/ColorPalette';
import { colorShades } from '@/lib/ui/color';

const HUES = {
  Grey: '#6b7079', Graphite: '#292b32', Brown: '#7d5a46', Indigo: '#5e6ad2',
  Teal: '#3b8b94', Blue: '#3b82b6', Violet: '#8a5cc7', Pink: '#c2569f', Rose: '#c35b72',
  Green: '#438b6b', Lime: '#6f9a3a', Amber: '#b78835', Orange: '#d0702f', Red: '#c8483f',
};
type Hue = keyof typeof HUES;

/** A hue picks its base for strokes and its palest shade for fills; both show the same shades. */
const hue = (name: Hue, fill = false): PaletteColor => {
  const shades = colorShades(HUES[name])!;
  return { name, color: fill ? shades[0] : HUES[name], shades };
};
const WHITE: PaletteColor = { name: 'White', color: '#ffffff' };
const ROWS: Hue[] = ['Teal', 'Blue', 'Violet', 'Pink', 'Rose', 'Green', 'Lime', 'Amber', 'Orange', 'Red'];

export const STROKE_PALETTE: PaletteColor[] = [WHITE, ...(['Grey', 'Graphite', 'Brown', 'Indigo', ...ROWS] as Hue[]).map(name => hue(name))];
export const FILL_PALETTE: PaletteColor[] = [{ name: 'None', color: 'transparent' }, WHITE, ...(['Grey', 'Brown', 'Indigo', ...ROWS] as Hue[]).map(name => hue(name, true))];

const pick = (palette: PaletteColor[], names: string[]) => names.map(name => palette.find(c => c.name === name)!);
/** The row shown in the panel, beside the button for the whole palette. */
export const STROKE_PICKS = pick(STROKE_PALETTE, ['Graphite', 'Red', 'Orange', 'Amber', 'Green', 'Teal', 'Blue', 'Violet', 'Pink']);
/** Fluorescent inks for the highlighter, which the muted palette has none of. */
export const MARKER_PICKS: PaletteColor[] = [
  { name: 'Yellow', color: '#ffe14d' }, { name: 'Green', color: '#8be28f' }, { name: 'Blue', color: '#7cc4ff' },
  { name: 'Pink', color: '#ff8fc7' }, { name: 'Orange', color: '#ffb061' }, { name: 'Purple', color: '#c3a3ff' },
];
export const FILL_PICKS = pick(FILL_PALETTE, ['None', 'Red', 'Green', 'Blue', 'Amber']);
