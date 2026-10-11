// Prop builders: each serialises (toJSON) to what Rust's schema reads.
const builder = (spec) => ({
  __prop: true,
  spec,
  default(v) {
    return builder(Object.assign({}, spec, { default: v }));
  },
  optional() {
    return builder(Object.assign({}, spec, { optional: true }));
  },
  toJSON() {
    return spec;
  },
});
define('str', () => builder({ kind: 'str', optional: false }));
define('num', () => builder({ kind: 'num', optional: false }));
define('bool', () => builder({ kind: 'bool', optional: false }));
define('latex', () => builder({ kind: 'latex', optional: false }));
define('list', (of) => {
  if (!of || !of.__prop) throw new TypeError('list(x) needs a prop builder, e.g. list(str())');
  return builder({ kind: 'list', optional: false, of: of.spec });
});
define('oneOf', (values) => {
  if (!Array.isArray(values)) throw new TypeError('oneOf takes an array, e.g. oneOf(["left", "right"])');
  return builder({ kind: 'oneOf', optional: false, values });
});
define('token', (kind) => {
  if (typeof kind !== 'string') throw new TypeError('token(kind) takes a theme section name, e.g. token("color")');
  return builder({ kind: 'token', optional: false, token: kind });
});
