import { defaultSchema, type Options } from "rehype-sanitize";

/** remark-math's markers, which `rehypeMaths` finds its formulas by. */
const MATH_CLASSES = ["math-inline", "math-display", "language-math"];

const base = defaultSchema.attributes ?? {};

/**
 * What raw HTML in a library file may render (docs/viewers.md). GitHub's
 * schema, plus the tags the Canvas converter emits (`<details>`, `<echo-video>`)
 * and remark-math's classes.
 */
export const SANITIZE_SCHEMA: Options = {
  ...defaultSchema,
  tagNames: [...(defaultSchema.tagNames ?? []), "echo-video"],
  attributes: {
    ...base,
    code: [["className", /^language-./, ...MATH_CLASSES]],
    span: [...(base.span ?? []), ["className", ...MATH_CLASSES]],
    div: [...(base.div ?? []), ["className", ...MATH_CLASSES]],
    pre: [...(base.pre ?? []), ["className", ...MATH_CLASSES]],
    "echo-video": ["link-id", "src"],
  },
};
