import { installMaths, mathsReady } from "./engine";

// Vite emits both as assets; bun resolves them as file URLs.
const WASM = new URL("../../../math-core/pkg/oculus_math_bg.wasm", import.meta.url);
const GLUE = new URL("../../../math-core/pkg/oculus_math.js", import.meta.url);

let loading: Promise<void> | null = null;

async function fetched(url: URL): Promise<Response> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`Could not load the maths engine: ${res.status} for ${url}`);
  return res;
}

/** Fetches, compiles and instantiates the maths engine, once. Started at boot
 *  (`main.tsx`) without waiting; until it resolves `mathsReady()` is false. */
export function loadMaths(): Promise<void> {
  loading ??= (async () => {
    if (mathsReady()) return;
    const [bytes, source] = await Promise.all([
      fetched(WASM).then((r) => r.arrayBuffer()),
      fetched(GLUE).then((r) => r.text()),
    ]);
    await installMaths(await WebAssembly.compile(bytes), source);
  })();
  return loading;
}
