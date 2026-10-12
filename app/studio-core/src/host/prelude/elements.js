// The JSX factory and the host elements. Box, Text, Image, Path, Morph and Math
// are host elements, checked as they are made; any other type is a function
// component, which `h` defers so the resolver (resolve.js) can call it top-down
// with its subtree's ctx. `<Math>` names JS's own Math object, so `h` knows it by
// identity and Math.max and the rest stay as they are. The prelude files share one module scope, in this order:
// elements, resolve, props, motion, time, presets, arrow, words, entry.
const g = globalThis;
const define = (name, value) =>
  Object.defineProperty(g, name, { value, writable: false, enumerable: false, configurable: false });

// Private marks: a deferred component element, and a subtree with its own ctx.
const COMPONENT = Symbol('component');
const SCOPE = Symbol('scope');

const NAMES = { box: 'Box', text: 'Text', image: 'Image', path: 'Path', morph: 'Morph', math: 'Math' };
const MATH = g.Math;
const elementName = (k) => NAMES[k] || String(k);
const componentName = (fn) => (fn && fn.name) || 'component';
const describe = (v) => {
  if (v === null) return 'null';
  if (Array.isArray(v)) return 'an array';
  if (typeof v === 'object') {
    if (v[COMPONENT]) return `a <${componentName(v.fn)}> element`;
    if (v[SCOPE]) return 'an element';
    return v.k ? `a <${elementName(v.k)}> element` : 'an object';
  }
  if (typeof v === 'function') return 'a function';
  if (typeof v === 'string') return JSON.stringify(v);
  return String(v);
};

const ALLOWED = {
  box: ['style', 'key', 'children'],
  text: ['style', 'key', 'children'],
  image: ['style', 'key', 'src'],
  path: ['style', 'key', 'd', 'viewBox', 'fill', 'stroke', 'trim'],
  morph: ['p', 'children'],
  math: ['style', 'key', 'tex', 'display'],
};
const PATH_PROPS = ['viewBox', 'fill', 'stroke', 'trim'];
const IGNORED = ['__source', '__self'];
const flatten = (children, out) => {
  for (const c of children) {
    if (c == null || c === false || c === true) continue;
    if (Array.isArray(c)) flatten(c, out);
    else out.push(c);
  }
  return out;
};
const isObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
const isHost = (v) => isObject(v) && typeof v.k === 'string';
const isComponent = (v) => isObject(v) && v[COMPONENT] === true;
const isScope = (v) => isObject(v) && v[SCOPE] === true;
const isElement = (v) => isHost(v) || isComponent(v) || isScope(v);
const locOf = (props) => {
  const src = props.__source;
  return src && typeof src.lineNumber === 'number' ? [src.lineNumber, src.columnNumber || 0] : undefined;
};

function h(type, props, ...children) {
  props = props || {};
  const kids = flatten(children, []);
  if (type === MATH) type = 'math';
  if (typeof type === 'function') {
    const p = Object.assign({}, props);
    if (kids.length) p.children = kids;
    const node = { fn: type, props: p, loc: locOf(props) };
    if (props.key !== undefined && props.key !== null) node.key = String(props.key);
    node[COMPONENT] = true;
    return Object.freeze(node);
  }
  const allowed = ALLOWED[type];
  if (!allowed) {
    throw new TypeError(`unknown element ${describe(type)}; use Box, Text, Image, Path, Math, Stack, Row or a function component`);
  }
  const name = elementName(type);
  for (const key of Object.keys(props)) {
    if (allowed.includes(key) || IGNORED.includes(key)) continue;
    const hint = type === 'image' || type === 'math' || key === 'src' ? '' : ` (style keys go inside style={{ ${key}: ... }})`;
    throw new TypeError(`<${name}> has no prop "${key}"; it takes ${allowed.join(', ')}${hint}`);
  }
  const node = { k: type };
  if (props.style !== undefined) {
    if (props.style === null || typeof props.style !== 'object' || Array.isArray(props.style)) {
      throw new TypeError(`<${name}> style must be an object, got ${describe(props.style)}`);
    }
    node.style = props.style;
  }
  if (props.key !== undefined && props.key !== null) node.key = String(props.key);
  const loc = locOf(props);
  if (loc) node.loc = loc;
  if (type === 'text') {
    for (const c of kids) {
      if (typeof c === 'string') continue;
      if (typeof c === 'number' && Number.isFinite(c)) continue;
      throw new TypeError(
        `<Text> children must be strings or numbers, got ${describe(c)}; to mix styles put several <Text> in a <Row>`
      );
    }
    node.text = kids.join('');
  } else if (type === 'image') {
    if (kids.length) throw new TypeError('<Image> takes no children');
    if (typeof props.src !== 'string' || !props.src) {
      throw new TypeError(`<Image> needs src="file.png" (relative to assets/), got ${describe(props.src)}`);
    }
    node.src = props.src;
  } else if (type === 'path') {
    if (kids.length) throw new TypeError('<Path> takes no children');
    if (typeof props.d !== 'string') {
      throw new TypeError(`<Path> needs d="M 0 0 L 10 10" (SVG path data), got ${describe(props.d)}`);
    }
    node.d = props.d;
    for (const k of PATH_PROPS) if (props[k] !== undefined) node[k] = props[k];
  } else if (type === 'math') {
    // Rust typesets it with KaTeX and expands it into keyed glyphs.
    if (kids.length) throw new TypeError('<Math> takes no children; put the LaTeX in tex="..."');
    if (typeof props.tex !== 'string') {
      throw new TypeError(`<Math> needs tex="..." (LaTeX source), got ${describe(props.tex)}`);
    }
    if (props.display !== undefined && typeof props.display !== 'boolean') {
      throw new TypeError(`<Math> display must be true or false, got ${describe(props.display)}`);
    }
    node.tex = props.tex;
    if (props.display !== undefined) node.display = props.display;
  } else if (type === 'morph') {
    // Rust lays the tree out with each child in the Morph's place and
    // blends the two layouts at p.
    if (typeof props.p !== 'number' || !Number.isFinite(props.p)) {
      throw new TypeError(`<Morph> needs p={0..1}, a finite number, got ${describe(props.p)}`);
    }
    if (kids.length !== 2 || !kids.every(isElement)) {
      throw new TypeError(`<Morph> takes exactly two elements, the layout at p = 0 and the layout at p = 1, got ${kids.length === 2 ? kids.map(describe).join(' and ') : kids.length}`);
    }
    node.p = clamp(props.p, 0, 1);
    node.children = kids;
  } else {
    for (const c of kids) {
      if (!isElement(c)) {
        throw new TypeError(`<Box> children must be elements, got ${describe(c)}; put text inside <Text>`);
      }
    }
    node.children = kids;
  }
  return node;
}
define('__studio_h', h);
define('Box', 'box');
define('Text', 'text');
define('Image', 'image');
define('Path', 'path');
define('Morph', 'morph');
const Fragment = function Fragment(props) {
  return (props && props.children) || [];
};
define('Fragment', Fragment);
define('__studio_Fragment', Fragment);

const named = (name, fn) => Object.defineProperty(fn, 'name', { value: name });
// Rejects props a built-in component does not take.
const onlyProps = (name, rest, takes) => {
  const extra = Object.keys(rest);
  if (extra.length) throw new TypeError(`<${name}> has no prop "${extra[0]}"; it takes ${takes}`);
};

// Stack (column) and Row (row): a Box with layout shorthands.
const flex = (direction, name) =>
  named(name, function (props) {
    const { gap, align, justify, wrap, style, key, children, __source, __self, ...rest } = props;
    onlyProps(name, rest, 'gap, align, justify, wrap, style, key, children');
    const s = { flexDirection: direction };
    if (gap !== undefined) s.gap = gap;
    if (align !== undefined) s.alignItems = align;
    if (justify !== undefined) s.justifyContent = justify;
    if (wrap) s.flexWrap = 'wrap';
    return h('box', { key, __source, style: Object.assign(s, style) }, children);
  });
define('Stack', flex('column', 'Stack'));
define('Row', flex('row', 'Row'));
