/** Mermaid is large and rarely needed, so it loads lazily on the first fence;
 *  the promise is shared. */
let loading: Promise<typeof import("mermaid").default> | null = null;

export function load() {
  loading ??= import("mermaid").then((m) => m.default);
  return loading;
}
