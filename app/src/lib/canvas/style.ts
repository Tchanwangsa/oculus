import { CANVAS_DEFAULTS, LABELLED_TYPES, roundnessOf, type CanvasElement, type CanvasTextAlign } from './model';

/** Appearance fields the inspector edits; everything else on an element is geometry or content. */
export const STYLE_KEYS = [
  'stroke', 'fill', 'strokeWidth', 'style', 'opacity', 'strokeStyle',
  'fontFamily', 'fontSize', 'textAlign', 'verticalAlign', 'startArrowhead', 'endArrowhead', 'roundness', 'arrowType',
] as const;
export type StyleKey = typeof STYLE_KEYS[number];
export type StylePatch = Partial<Pick<CanvasElement, StyleKey>>;
export type StyleValues = Required<Pick<CanvasElement, StyleKey>>;
/** Defaults for new elements. Shape labels keep their own alignment so new text can start left while labels centre. */
export type DrawingStyle = StyleValues & { labelAlign: CanvasTextAlign };

type ElementType = CanvasElement['type'];
export const DEFAULT_DRAWING_STYLE: DrawingStyle = {
  stroke: '#292b32', fill: 'transparent', strokeWidth: 2, style: 'sketch', opacity: CANVAS_DEFAULTS.opacity,
  strokeStyle: CANVAS_DEFAULTS.strokeStyle, fontFamily: CANVAS_DEFAULTS.fontFamily, fontSize: CANVAS_DEFAULTS.fontSize,
  textAlign: CANVAS_DEFAULTS.textAlign, labelAlign: CANVAS_DEFAULTS.labelAlign, verticalAlign: CANVAS_DEFAULTS.verticalAlign,
  startArrowhead: CANVAS_DEFAULTS.startArrowhead, endArrowhead: CANVAS_DEFAULTS.endArrowhead, roundness: CANVAS_DEFAULTS.roundness,
  arrowType: CANVAS_DEFAULTS.arrowType,
};

const ALL: ElementType[] = ['rectangle', 'ellipse', 'diamond', 'line', 'arrow', 'freedraw', 'text'];
const SHAPES: ElementType[] = ['rectangle', 'ellipse', 'diamond'];
const OUTLINED: ElementType[] = [...SHAPES, 'line', 'arrow'];
const TEXTUAL: ElementType[] = ['text', ...SHAPES];
const KEY_TYPES: Record<StyleKey, readonly ElementType[]> = {
  stroke: ALL, opacity: [...ALL, 'image'], fill: SHAPES, strokeWidth: ALL.filter(type => type !== 'text'), strokeStyle: OUTLINED, style: OUTLINED,
  fontFamily: TEXTUAL, fontSize: TEXTUAL, textAlign: TEXTUAL, verticalAlign: SHAPES, startArrowhead: ['arrow'], endArrowhead: ['arrow'],
  roundness: ['rectangle', 'diamond'], arrowType: ['arrow'],
};
const LABEL_KEYS: ReadonlySet<StyleKey> = new Set(['fontFamily', 'fontSize', 'textAlign', 'verticalAlign']);

/** Whether a patch may write this field to an element of this type. */
export const appliesTo = (key: StyleKey, type: ElementType) => KEY_TYPES[key].includes(type);

/** What an element without the field draws with; base fields are always present. */
export function defaultValue<K extends StyleKey>(key: K, type: ElementType): StyleValues[K] {
  if (key === 'textAlign') return (type === 'text' ? CANVAS_DEFAULTS.textAlign : CANVAS_DEFAULTS.labelAlign) as StyleValues[K];
  if (key === 'roundness') return roundnessOf({ type } as CanvasElement) as StyleValues[K];
  return (CANVAS_DEFAULTS as Record<string, unknown>)[key] as StyleValues[K];
}
export const elementValue = <K extends StyleKey>(el: CanvasElement, key: K): StyleValues[K] => (el[key] ?? defaultValue(key, el.type)) as StyleValues[K];
const styleValue = <K extends StyleKey>(style: DrawingStyle, key: K, type: ElementType): StyleValues[K] =>
  (key === 'textAlign' && type !== 'text' ? style.labelAlign : style[key]) as StyleValues[K];

/** Base fields always; optional ones only where they apply and differ from the default, so files stay small. */
export function newElementStyle(type: ElementType, style: DrawingStyle): StylePatch & Pick<CanvasElement, 'stroke' | 'fill' | 'strokeWidth' | 'style'> {
  const out: StylePatch & Pick<CanvasElement, 'stroke' | 'fill' | 'strokeWidth' | 'style'> = {
    stroke: style.stroke, fill: appliesTo('fill', type) ? style.fill : 'transparent', strokeWidth: style.strokeWidth, style: style.style,
  };
  for (const key of STYLE_KEYS) {
    if (key in out || !appliesTo(key, type)) continue;
    const value = styleValue(style, key, type);
    if (value !== defaultValue(key, type)) Object.assign(out, { [key]: value });
  }
  return out;
}

/** Selecting an element makes its look the default for the next one drawn. */
export function adoptElementStyle(style: DrawingStyle, el: CanvasElement): DrawingStyle {
  const next = { ...style };
  for (const key of STYLE_KEYS) {
    if (!appliesTo(key, el.type)) continue;
    const value = elementValue(el, key);
    Object.assign(next, { [key === 'textAlign' && el.type !== 'text' ? 'labelAlign' : key]: value });
  }
  return next;
}

/** A patch from the inspector also becomes the default for the kinds it was made on. */
export function patchDrawingStyle(style: DrawingStyle, patch: StylePatch, types: Iterable<ElementType>): DrawingStyle {
  const { textAlign, ...rest } = patch;
  const next = { ...style, ...rest } as DrawingStyle;
  if (textAlign) {
    const kinds = [...types];
    if (!kinds.length || kinds.includes('text')) next.textAlign = textAlign;
    if (!kinds.length || kinds.some(type => LABELLED_TYPES.has(type))) next.labelAlign = textAlign;
  }
  return next;
}

/** Writes only fields that apply to the element and change what it draws; returns the same object when nothing does. */
export function patchElementStyle(el: CanvasElement, patch: StylePatch): CanvasElement {
  let next = el;
  for (const key of STYLE_KEYS) {
    const value = patch[key];
    if (value === undefined || !appliesTo(key, el.type) || elementValue(el, key) === value) continue;
    next = { ...next, [key]: value };
  }
  return next;
}

export type InspectorSection = 'stroke' | 'fill' | 'text' | 'verticalAlign' | 'strokeWidth' | 'strokeStyle' | 'sketch' | 'roundness' | 'arrowType' | 'arrowheads' | 'opacity';
const SECTION_KEYS: [InspectorSection, StyleKey][] = [
  ['stroke', 'stroke'], ['fill', 'fill'], ['text', 'fontSize'], ['verticalAlign', 'verticalAlign'], ['strokeWidth', 'strokeWidth'],
  ['strokeStyle', 'strokeStyle'], ['sketch', 'style'], ['roundness', 'roundness'], ['arrowType', 'arrowType'], ['arrowheads', 'endArrowhead'], ['opacity', 'opacity'],
];

/** What the inspector describes: a selected element, an element being written in, or the active tool's next element. */
export interface InspectorTarget { type: ElementType; label: boolean; element?: CanvasElement }
export const inspectorTarget = (el: CanvasElement, label = LABELLED_TYPES.has(el.type) && Boolean(el.text?.trim())): InspectorTarget => ({ type: el.type, label, element: el });

/** Text options show on a shape only once it has a label (or one is being written). */
const shows = (key: StyleKey, target: InspectorTarget) => appliesTo(key, target.type) && (!LABEL_KEYS.has(key) || target.type === 'text' || target.label);

/** The union of what applies to any target, in panel order. */
export function inspectorSections(targets: readonly InspectorTarget[]): InspectorSection[] {
  return SECTION_KEYS.filter(([, key]) => targets.some(target => shows(key, target))).map(([section]) => section);
}

/** Each field reads from the first target that shows it, or the tool's defaults when nothing is selected. */
export function inspectorValues(targets: readonly InspectorTarget[], style: DrawingStyle): StyleValues {
  const values = {} as StyleValues;
  for (const key of STYLE_KEYS) {
    const target = targets.find(item => shows(key, item)) ?? targets[0];
    Object.assign(values, { [key]: target?.element ? elementValue(target.element, key) : styleValue(style, key, target?.type ?? 'text') });
  }
  return values;
}

export const colorLabel = (targets: readonly InspectorTarget[]) => targets.length && targets.every(target => target.type === 'text') ? 'Color' : 'Stroke';
