import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// Rendering a Studio video folder to an mp4 with ffmpeg
// (`app/src-tauri/src/studio/commands/render.rs`). One render per folder at a
// time; nothing here touches the network.

/** Mirrors `render::Quality`. */
export type RenderQuality = "draft" | "final";

/** Mirrors `SegmentReport`: one per scene, in video order. */
export interface SegmentReport {
  /** The scene, counting from 0. */
  scene: number;
  id: string | null;
  outcome: "cached" | "rendered";
  frames: number;
  /** Rendering and encoding it; 0 when cached. */
  ms: number;
  /** Frames a second while rendering it; null when cached. */
  fps: number | null;
}

/** Mirrors `render::Report`. */
export interface RenderReport {
  /** The mp4 written. */
  path: string;
  quality: RenderQuality;
  width: number;
  height: number;
  fps: number;
  frames: number;
  /** Seconds. */
  duration: number;
  /** Whether it has a narration track. */
  audio: boolean;
  threads: number;
  segments: SegmentReport[];
  /** Cache files deleted afterwards. */
  pruned: string[];
  ms: number;
}

/** Renders `dir` to `<dir>/renders/<quality>.mp4`. Resolves to the report,
 *  or null when cancelled; rejects with a sentence fit to show when the
 *  render fails or `dir` already has one running. */
export function renderVideo(dir: string, quality: RenderQuality): Promise<RenderReport | null> {
  return invoke<RenderReport | null>("studio_render", { dir, quality });
}

/** false when `dir` has no render running. */
export function cancelRender(dir: string): Promise<boolean> {
  return invoke<boolean>("studio_render_cancel", { dir });
}

/** Mirrors `Plan`. */
export interface RenderPlan {
  segments: number;
  cached: number;
  /** Frames to render: the segments not cached. */
  frames: number;
  threads: number;
}

/** Mirrors `Event`; `dir` is the folder as `renderVideo` was given it.
 *  `rendering` comes at most about ten times a second, plus each segment's
 *  first and last frame. */
export type RenderProgress = { dir: string } & (
  | { phase: "planning"; plan?: RenderPlan }
  | {
      phase: "rendering";
      /** 1-based, of `of` segments. */
      segment: number;
      of: number;
      scene: number;
      /** 1-based, of this segment's `frames`. */
      frame: number;
      frames: number;
      fps: number;
    }
  | { phase: "muxing" }
  | { phase: "complete"; path: string; report: RenderReport }
  | { phase: "cancelled" }
  | { phase: "error"; error: string }
);

export function onRenderProgress(handler: (progress: RenderProgress) => void): Promise<UnlistenFn> {
  return listen<RenderProgress>("studio-render-progress", (event) => handler(event.payload));
}
