/** `react/jsx-runtime` for widgets: what the transpiler's automatic JSX
 *  runtime imports. Explicit for the same reason as `react.ts`. */
import * as runtime from "react/jsx-runtime";

export const { jsx, jsxs, Fragment } = runtime;
