/** `oculus/math`: mathjs (a `create()` subset) and `formula`, a chunk of its
 *  own that loads only when a widget imports it. The contract is
 *  `widget.d.ts`. */
import { math } from "./instance";

export { math };
export { formula, type Formula } from "./formula";

export const {
  parse,
  compile,
  evaluate,
  derivative,
  simplify,
  complex,
  matrix,
  eigs,
  det,
  inv,
  transpose,
  identity,
  zeros,
  dot,
  cross,
  norm,
  add,
  subtract,
  multiply,
  divide,
  pow,
  sqrt,
  exp,
  log,
  sin,
  cos,
  tan,
  atan2,
  abs,
  re,
  im,
  arg,
  conj,
  pi,
  e,
  i,
} = math;
