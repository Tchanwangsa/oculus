/**
 * `@openuidev/react-lang` mounts its OpenUI Inspect widget over the page in
 * dev builds unless this flag is already set when the package first loads.
 * Imported first in `main.tsx`, ahead of anything that reaches the package.
 */
(globalThis as Record<symbol, unknown>)[Symbol.for("openui.devtools.autoMount")] = true;
