/**
 * Whether the parent has the frame in view, from its `pause`/`resume`
 * messages (docs/widgets.md#a-3d-scene-releases-its-context-off-screen).
 * "unknown" until the first one: a 3D scene creates no WebGL context before
 * the parent says `resume`. Shared by the runtime and `oculus/3d` through one
 * chunk, so both see the same value.
 */
export type Visibility = "unknown" | "live" | "paused";

let visibility: Visibility = "unknown";
const listeners = new Set<() => void>();

export function getVisibility(): Visibility {
  return visibility;
}

export function subscribeVisibility(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function setVisibility(next: Visibility) {
  if (next === visibility) return;
  visibility = next;
  for (const listener of [...listeners]) listener();
}
