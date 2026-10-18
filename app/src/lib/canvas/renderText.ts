import { CANVAS_DEFAULTS, textAlignOf, type CanvasElement, type CanvasFontFamily, type CanvasTextAlign } from './model';
import type { Bounds } from './geometry';

/** sans and mono repeat `--font-sans` / `--font-mono` in `styles/theme.css`; Canvas2D and SVG cannot read CSS variables. */
export const CANVAS_FONT_STACKS: Record<CanvasFontFamily, string> = {
  sans: '"Inter Variable", "Inter", ui-sans-serif, system-ui, sans-serif',
  serif: 'ui-serif, "New York", Georgia, "Times New Roman", serif',
  mono: 'ui-monospace, "SF Mono", SFMono-Regular, Menlo, "Cascadia Mono", Consolas, monospace',
};
export const LINE_HEIGHT = 1.4;
export const LABEL_PADDING = 8;

export const canvasFont = (size: number, family: CanvasFontFamily = CANVAS_DEFAULTS.fontFamily) => `${size}px ${CANVAS_FONT_STACKS[family]}`;

export type TextMeasure = (line: string, fontSize: number, fontFamily: CanvasFontFamily) => number;
let measureContext: CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D | null | undefined;
/** One offscreen context measures for every renderer; without a canvas (tests) it estimates 0.6em per character. */
export const measureCanvasText: TextMeasure = (line, fontSize, fontFamily) => {
  measureContext ??= typeof OffscreenCanvas !== 'undefined' ? new OffscreenCanvas(1, 1).getContext('2d')
    : typeof document !== 'undefined' ? document.createElement('canvas').getContext('2d') : null;
  if (!measureContext) return line.length * fontSize * 0.6;
  measureContext.font = canvasFont(fontSize, fontFamily);
  return measureContext.measureText(line).width;
};

/** Greedy by words within each explicit line; a word wider than the box breaks between characters. */
export function wrapLines(text: string, maxWidth: number, measure: (line: string) => number): string[] {
  const lines: string[] = [];
  for (const paragraph of text.split('\n')) {
    let line = '';
    for (let word of paragraph.match(/\s*\S+/g) ?? ['']) {
      if (line && measure(line + word) <= maxWidth) { line += word; continue; }
      if (line) { lines.push(line); word = word.trimStart(); }
      let chars = Array.from(word);
      while (chars.length > 1 && measure(word) > maxWidth) {
        let n = 1;
        while (n < chars.length - 1 && measure(chars.slice(0, n + 1).join('')) <= maxWidth) n++;
        lines.push(chars.slice(0, n).join('')); chars = chars.slice(n); word = chars.join('');
      }
      line = word;
    }
    lines.push(line);
  }
  return lines;
}

export interface TextLine { text: string; x: number; y: number }
/** Line x is the alignment anchor (left edge, centre or right edge); y is the em box's top, half-leading included. */
export interface TextLayout { lines: TextLine[]; fontSize: number; fontFamily: CanvasFontFamily; textAlign: CanvasTextAlign; box: Bounds; height: number }

function placeLines(texts: string[], box: Bounds, el: CanvasElement, top: number): TextLayout {
  const fontSize = el.fontSize ?? CANVAS_DEFAULTS.fontSize, textAlign = textAlignOf(el), line = fontSize * LINE_HEIGHT;
  const x = textAlign === 'left' ? box.x : textAlign === 'center' ? box.x + box.width / 2 : box.x + box.width;
  const lines = texts.map((text, i) => ({ text, x, y: top + i * line + (line - fontSize) / 2 }));
  return { lines, fontSize, fontFamily: el.fontFamily ?? CANVAS_DEFAULTS.fontFamily, textAlign, box, height: texts.length * line };
}

/** Element-local; ellipses and diamonds use their inscribed box, then every shape loses the padding. */
export function labelBox(el: CanvasElement): Bounds {
  const w = Math.abs(el.width), h = Math.abs(el.height), scale = el.type === 'ellipse' ? Math.SQRT1_2 : el.type === 'diamond' ? 0.5 : 1;
  const width = Math.max(0, w * scale - LABEL_PADDING * 2), height = Math.max(0, h * scale - LABEL_PADDING * 2);
  return { x: Math.min(0, el.width) + (w - width) / 2, y: Math.min(0, el.height) + (h - height) / 2, width, height };
}

export function layoutShapeLabel(el: CanvasElement, measure: TextMeasure = measureCanvasText): TextLayout {
  const box = labelBox(el), fontSize = el.fontSize ?? CANVAS_DEFAULTS.fontSize, family = el.fontFamily ?? CANVAS_DEFAULTS.fontFamily;
  const texts = wrapLines(el.text ?? '', box.width, line => measure(line, fontSize, family));
  const height = texts.length * fontSize * LINE_HEIGHT, align = el.verticalAlign ?? CANVAS_DEFAULTS.verticalAlign;
  return placeLines(texts, box, el, align === 'top' ? box.y : align === 'bottom' ? box.y + box.height - height : box.y + (box.height - height) / 2);
}

/** Standalone text keeps its explicit lines and aligns them within its own measured width. */
export function layoutTextElement(el: CanvasElement): TextLayout {
  const box = { x: Math.min(0, el.width), y: Math.min(0, el.height), width: Math.abs(el.width), height: Math.abs(el.height) };
  return placeLines((el.text ?? '').split('\n'), box, el, box.y);
}

/** Standalone text sizes to its explicit lines in its own font: after an edit and after a size or family change. */
export function measureTextElement(el: CanvasElement, measure: TextMeasure = measureCanvasText): CanvasElement {
  const fontSize = el.fontSize ?? CANVAS_DEFAULTS.fontSize, family = el.fontFamily ?? CANVAS_DEFAULTS.fontFamily;
  const lines = (el.text ?? '').split('\n');
  return { ...el, width: Math.max(1, ...lines.map(line => measure(line, fontSize, family))), height: lines.length * fontSize * LINE_HEIGHT };
}
