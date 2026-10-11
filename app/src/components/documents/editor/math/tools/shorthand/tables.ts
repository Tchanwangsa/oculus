/** `@<key>` → Greek letter. */
export const GREEK: Record<string, string> = {
  a: "alpha", b: "beta", g: "gamma", G: "Gamma", d: "delta", D: "Delta",
  e: "epsilon", ve: "varepsilon", z: "zeta", h: "eta", t: "theta", T: "Theta",
  vt: "vartheta", i: "iota", k: "kappa", l: "lambda", L: "Lambda", m: "mu",
  n: "nu", x: "xi", X: "Xi", p: "pi", P: "Pi", r: "rho", s: "sigma",
  S: "Sigma", u: "upsilon", f: "phi", vf: "varphi", F: "Phi", c: "chi",
  y: "psi", Y: "Psi", o: "omega", O: "Omega",
};

/** Letters typed after a base (`x`, `2`, `)`, `\alpha`) → its superscript. */
export const POWERS: Record<string, string> = {
  sr: "^2",
  cb: "^3",
  rd: "^{#{1}}#{0}",
  invs: "^{-1}",
};

/** Symbol runs → operators, longest first. `\le>` is `<=` already expanded. */
export const OPERATORS: [string, string][] = [
  ["<->", "\\leftrightarrow"],
  ["<=>", "\\iff"],
  ["\\le>", "\\iff"],
  ["->", "\\to"],
  ["=>", "\\implies"],
  ["<=", "\\le"],
  [">=", "\\ge"],
  ["!=", "\\ne"],
  ["~~", "\\approx"],
  ["...", "\\dots"],
  ["**", "\\cdot"],
];

/** Letter runs → operators, only when the whole run is the trigger. */
export const WORD_OPERATORS: Record<string, string> = { ooo: "\\infty", xx: "\\times" };

/** Bare words that get a backslash once the next character ends them. */
export const FUNCTIONS = new Set([
  "sin", "cos", "tan", "sec", "csc", "cot", "arcsin", "arccos", "arctan",
  "sinh", "cosh", "tanh", "log", "ln", "exp", "lim", "limsup", "liminf",
  "max", "min", "det", "sup", "inf", "gcd", "sum", "prod", "int",
]);
/** Characters that end a function word. */
export const FUNCTION_ENDS = new Set([" ", "(", "^", "_", "\\"]);

/** Bare words that expand the moment they are complete, as snippets. */
export const SNIPPET_WORDS: Record<string, string> = { sqrt: "\\sqrt{#{1}}#{0}" };

/** `)` / `]` around one of these becomes `\left( … \right)`. */
export const TALL = /\\(?:[dt]?frac|binom|sum|prod|i?int|oint|begin)(?![A-Za-z])/;

/** Arguments that hold text or names, where no rule fires. */
export const TEXT_ARGUMENT =
  /\\(?:text(?:bf|it|tt|rm|sf|color)?|math(?:rm|it|sf|tt|cal|bb|frak|scr)|operatorname\*?|mbox|begin|end|label|(?:eq)?ref|tag|color|href|url)\s*$/;
