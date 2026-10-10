/**
 * What the maths popover and `\` completion offer (`tools/mathTools`). A
 * template is CodeMirror snippet syntax: each `#{}` is a Tab field, filled in
 * order. Templates stay on one line, since inline `$…$` cannot span lines.
 * A button's preview is the template with its fields drawn as `\square`,
 * unless the entry gives its own `label`.
 */

export interface MathEntry {
  template: string;
  /** LaTeX for the button preview, when the template's own reads badly. */
  label?: string;
  /** Takes two grid columns (tall or wide previews). */
  wide?: boolean;
}

export interface MathTab {
  id: string;
  label: string;
  entries: MathEntry[];
}

const FIELD = /[#$]\{[^{}]*\}/g;

/** The LaTeX a button or completion row renders. */
export function previewOf(entry: MathEntry): string {
  return entry.label ?? entry.template.replace(FIELD, "\\square");
}

/** The fields numbered in order with a final stop after the template, so Tab
 *  past the last field leaves the snippet (and a field-less one still puts
 *  the caret after itself). */
export function snippetTemplate(template: string): string {
  let n = 0;
  return `${template.replace(FIELD, () => `\${${++n}}`)}\${0}`;
}

const plain = (...commands: string[]): MathEntry[] => commands.map((template) => ({ template }));

export const MATH_TABS: MathTab[] = [
  {
    id: "structures",
    label: "Structures",
    entries: [
      { template: "\\frac{#{}}{#{}}" },
      { template: "\\sqrt{#{}}" },
      { template: "\\sqrt[#{}]{#{}}" },
      { template: "^{#{}}", label: "x^{\\square}" },
      { template: "_{#{}}", label: "x_{\\square}" },
      { template: "_{#{}}^{#{}}", label: "x_{\\square}^{\\square}" },
      { template: "\\left( #{} \\right)" },
      { template: "\\left[ #{} \\right]" },
      { template: "\\left\\{ #{} \\right\\}" },
      { template: "\\left| #{} \\right|" },
      { template: "\\left\\lVert #{} \\right\\rVert" },
      { template: "\\lfloor #{} \\rfloor" },
      { template: "\\lceil #{} \\rceil" },
      { template: "\\binom{#{}}{#{}}" },
      { template: "\\overline{#{}}" },
      { template: "\\text{#{}}", label: "\\text{abc}" },
      { template: "\\begin{cases} #{} & #{} \\\\ #{} & #{} \\end{cases}", wide: true },
      { template: "\\begin{aligned} #{} &= #{} \\\\ &= #{} \\end{aligned}", wide: true },
    ],
  },
  {
    id: "greek",
    label: "Greek",
    entries: plain(
      "\\alpha", "\\beta", "\\gamma", "\\delta", "\\epsilon", "\\varepsilon",
      "\\zeta", "\\eta", "\\theta", "\\vartheta", "\\iota", "\\kappa",
      "\\lambda", "\\mu", "\\nu", "\\xi", "\\pi", "\\rho",
      "\\sigma", "\\tau", "\\upsilon", "\\phi", "\\varphi", "\\chi",
      "\\psi", "\\omega", "\\Gamma", "\\Delta", "\\Theta", "\\Lambda",
      "\\Xi", "\\Pi", "\\Sigma", "\\Phi", "\\Psi", "\\Omega",
    ),
  },
  {
    id: "relations",
    label: "Relations",
    entries: plain(
      "\\neq", "\\approx", "\\equiv", "\\sim", "\\cong", "\\propto",
      "\\leq", "\\geq", "\\ll", "\\gg", "\\in", "\\notin",
      "\\subset", "\\subseteq", "\\cup", "\\cap", "\\setminus", "\\emptyset",
      "\\pm", "\\times", "\\cdot", "\\div", "\\circ", "\\mid",
    ),
  },
  {
    id: "logic",
    label: "Arrows and logic",
    entries: plain(
      "\\to", "\\leftarrow", "\\leftrightarrow", "\\Rightarrow", "\\Leftarrow", "\\Leftrightarrow",
      "\\implies", "\\iff", "\\mapsto", "\\uparrow", "\\downarrow", "\\rightleftharpoons",
      "\\forall", "\\exists", "\\nexists", "\\neg", "\\land", "\\lor",
      "\\oplus", "\\therefore", "\\because", "\\vdash", "\\models", "\\top",
    ),
  },
  {
    id: "calculus",
    label: "Calculus",
    entries: [
      { template: "\\int #{} \\, d#{}" },
      { template: "\\int_{#{}}^{#{}} #{} \\, d#{}", wide: true },
      { template: "\\iint_{#{}} #{} \\, dA" },
      { template: "\\oint_{#{}} #{}" },
      { template: "\\sum_{#{}}^{#{}} #{}" },
      { template: "\\prod_{#{}}^{#{}} #{}" },
      { template: "\\lim_{#{} \\to #{}} #{}", wide: true },
      { template: "\\frac{d}{d#{}}" },
      { template: "\\frac{d#{}}{d#{}}" },
      { template: "\\frac{d^2 #{}}{d#{}^2}" },
      { template: "\\frac{\\partial #{}}{\\partial #{}}" },
      { template: "\\partial" },
      { template: "\\nabla" },
      { template: "\\infty" },
      { template: "\\left. #{} \\right|_{#{}}^{#{}}" },
      { template: "\\dot{#{}}" },
      { template: "\\ddot{#{}}" },
      { template: "\\frac{\\partial^2 #{}}{\\partial #{}^2}" },
    ],
  },
  {
    id: "stats",
    label: "Stats",
    entries: [
      { template: "\\mathbb{E}[#{}]" },
      { template: "\\operatorname{Var}(#{})", wide: true },
      { template: "\\operatorname{Cov}(#{}, #{})", wide: true },
      { template: "P(#{})" },
      { template: "P(#{} \\mid #{})", wide: true },
      { template: "\\mathcal{N}(#{}, #{})", wide: true },
      { template: "\\operatorname{Bin}(#{}, #{})", wide: true },
      { template: "\\operatorname{Poisson}(#{})", wide: true },
      { template: "\\overset{\\text{iid}}{\\sim}" },
      { template: "\\bar{#{}}" },
      { template: "\\hat{#{}}" },
      { template: "\\sum_{i=1}^{n} #{}" },
      { template: "\\frac{1}{n} \\sum_{i=1}^{n} #{}", wide: true },
      { template: "\\sigma^2" },
      { template: "\\chi^2" },
      { template: "\\mathbf{1}_{\\{#{}\\}}" },
      { template: "\\xrightarrow{#{}}" },
    ],
  },
  {
    id: "matrices",
    label: "Matrices",
    entries: [
      { template: "^{\\top}", label: "A^{\\top}" },
      { template: "^{-1}", label: "A^{-1}" },
      { template: "^{\\dagger}", label: "A^{\\dagger}" },
      { template: "\\det(#{})" },
      { template: "\\operatorname{tr}(#{})" },
      { template: "\\operatorname{rank}(#{})", wide: true },
      { template: "\\langle #{}, #{} \\rangle" },
      { template: "\\mathbf{#{}}", label: "\\mathbf{v}" },
      { template: "\\vec{#{}}" },
      { template: "I_{#{}}", label: "I_n" },
      { template: "\\otimes" },
      { template: "\\cdots" },
      { template: "\\vdots" },
      { template: "\\ddots" },
    ],
  },
];

export type MatrixKind = "pmatrix" | "bmatrix" | "vmatrix";

/** A rows × cols matrix with every cell a field. */
export function matrixTemplate(kind: MatrixKind, rows: number, cols: number): MathEntry {
  const row = Array.from({ length: cols }, () => "#{}").join(" & ");
  const body = Array.from({ length: rows }, () => row).join(" \\\\ ");
  return { template: `\\begin{${kind}} ${body} \\end{${kind}}`, wide: true };
}

/**
 * Commands `\` completion offers beyond the palette's own templates. A
 * command taking arguments carries its fields.
 */
export const MATH_COMMANDS: MathEntry[] = [
  // Greek variants and letters the palette leaves out
  ...plain("\\omicron", "\\varpi", "\\varrho", "\\varsigma", "\\Upsilon", "\\digamma"),
  // Operators and relations
  ...plain(
    "\\ast", "\\star", "\\bullet", "\\wedge", "\\vee", "\\bigcup", "\\bigcap", "\\bigoplus",
    "\\le", "\\ge", "\\ne", "\\prec", "\\succ", "\\preceq", "\\succeq", "\\simeq", "\\asymp",
    "\\supset", "\\supseteq", "\\ni", "\\perp", "\\parallel", "\\angle", "\\triangle", "\\degree",
    "\\varnothing", "\\aleph", "\\hbar", "\\ell", "\\Re", "\\Im", "\\wp", "\\prime",
    "\\ldots", "\\dots", "\\quad", "\\qquad",
    "\\gets", "\\rightarrow", "\\longrightarrow", "\\Longrightarrow", "\\longmapsto",
    "\\hookrightarrow", "\\rightharpoonup", "\\leadsto", "\\uparrow", "\\Uparrow", "\\Downarrow",
    "\\lnot", "\\bot", "\\square", "\\checkmark",
  ),
  { template: "\\int" },
  { template: "\\sum" },
  { template: "\\prod" },
  { template: "\\lim" },
  { template: "\\limsup" },
  { template: "\\liminf" },
  { template: "\\sup" },
  { template: "\\inf" },
  // Accents and decorations
  { template: "\\hat{#{}}" },
  { template: "\\widehat{#{}}" },
  { template: "\\tilde{#{}}" },
  { template: "\\widetilde{#{}}" },
  { template: "\\underline{#{}}" },
  { template: "\\overbrace{#{}}^{#{}}" },
  { template: "\\underbrace{#{}}_{#{}}" },
  { template: "\\overset{#{}}{#{}}" },
  { template: "\\underset{#{}}{#{}}" },
  { template: "\\stackrel{#{}}{#{}}" },
  { template: "\\cancel{#{}}" },
  { template: "\\boxed{#{}}" },
  // Fonts and text
  { template: "\\mathbb{#{}}", label: "\\mathbb{R}" },
  { template: "\\mathcal{#{}}", label: "\\mathcal{L}" },
  { template: "\\mathbf{#{}}", label: "\\mathbf{x}" },
  { template: "\\mathrm{#{}}", label: "\\mathrm{d}" },
  { template: "\\mathit{#{}}", label: "\\mathit{x}" },
  { template: "\\mathsf{#{}}", label: "\\mathsf{x}" },
  { template: "\\mathfrak{#{}}", label: "\\mathfrak{g}" },
  { template: "\\boldsymbol{#{}}", label: "\\boldsymbol{\\beta}" },
  { template: "\\operatorname{#{}}", label: "\\operatorname{op}" },
  { template: "\\textbf{#{}}", label: "\\textbf{abc}" },
  // Functions
  ...plain(
    "\\sin", "\\cos", "\\tan", "\\sec", "\\csc", "\\cot", "\\arcsin", "\\arccos", "\\arctan",
    "\\sinh", "\\cosh", "\\tanh", "\\log", "\\ln", "\\exp", "\\max", "\\min", "\\arg",
    "\\argmax", "\\argmin", "\\deg", "\\gcd", "\\ker", "\\dim", "\\Pr",
  ),
  { template: "\\log_{#{}}" },
  { template: "\\pmod{#{}}" },
  { template: "\\bmod" },
  { template: "\\dfrac{#{}}{#{}}" },
  { template: "\\tfrac{#{}}{#{}}" },
  { template: "\\cfrac{#{}}{#{}}" },
  { template: "\\binom{#{}}{#{}}" },
  // Spacing and brackets
  ...plain("\\langle", "\\rangle", "\\lvert", "\\rvert", "\\lVert", "\\rVert"),
  { template: "\\big(" },
  { template: "\\Big(" },
  { template: "\\bigg(" },
  // Environments
  { template: "\\begin{aligned} #{} &= #{} \\end{aligned}" },
  { template: "\\begin{cases} #{} & #{} \\\\ #{} & #{} \\end{cases}" },
  { template: "\\begin{pmatrix} #{} & #{} \\\\ #{} & #{} \\end{pmatrix}" },
  { template: "\\begin{bmatrix} #{} & #{} \\\\ #{} & #{} \\end{bmatrix}" },
  { template: "\\begin{vmatrix} #{} & #{} \\\\ #{} & #{} \\end{vmatrix}" },
  { template: "\\begin{matrix} #{} & #{} \\\\ #{} & #{} \\end{matrix}" },
  { template: "\\begin{array}{#{}} #{} \\end{array}", label: "\\begin{array}{cc} a & b \\end{array}" },
  { template: "\\begin{gathered} #{} \\\\ #{} \\end{gathered}" },
];

/** Commands typed most, ranked first among equally good matches. */
export const COMMON_COMMANDS = new Set([
  "\\frac", "\\sqrt", "\\sum", "\\int", "\\lim", "\\alpha", "\\beta", "\\theta", "\\lambda",
  "\\mu", "\\sigma", "\\pi", "\\infty", "\\partial", "\\left", "\\mathbb", "\\text", "\\cdot",
  "\\times", "\\leq", "\\geq", "\\neq", "\\to", "\\in", "\\hat", "\\bar", "\\vec", "\\begin",
]);

/** The Popular tab's cells before there is history to rank (`mathUsage.ts`):
 *  palette templates, most useful first. */
export const POPULAR_DEFAULTS = [
  "\\frac{#{}}{#{}}", "^{#{}}", "_{#{}}", "\\sqrt{#{}}", "\\left( #{} \\right)", "\\sum_{#{}}^{#{}} #{}",
  "\\int #{} \\, d#{}", "\\lim_{#{} \\to #{}} #{}", "\\alpha", "\\beta", "\\theta", "\\lambda", "\\mu",
  "\\sigma", "\\pi", "\\infty", "\\partial", "\\cdot", "\\times", "\\leq", "\\geq", "\\neq", "\\to",
  "\\in", "\\text{#{}}",
];
