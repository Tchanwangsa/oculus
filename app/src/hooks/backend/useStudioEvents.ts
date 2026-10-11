import { useTauriEvent } from "@/hooks/backend/useEvents";
import type { RenderProgress } from "@/lib/studio/render";
import { useRenderStore } from "@/stores/studio/renderStore";

/** Render progress into `renderStore`, for the rail's dot and each page's bar. */
export function useStudioEvents() {
  useTauriEvent<RenderProgress>("studio-render-progress", (e) => useRenderStore.getState().apply(e.payload));
}
