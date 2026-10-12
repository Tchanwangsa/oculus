import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// Rendering a Studio video folder to one export preset's file with ffmpeg
// (`app/src-tauri/src/studio/commands/render.rs`). One render per folder at a
// time; nothing here touches the network.

/** Mirrors `render::Preset`. */
export type RenderPreset = "draft" | "final" | "small";

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
  /** The file written: the mp4. */
  path: string;
  preset: RenderPreset;
  width: number | null;
  height: number | null;
  fps: number;
  frames: number;
  /** Seconds. */
  duration: number;
  /** Whether it has a narration track. */
  audio: boolean;
  threads: number;
  segments: SegmentReport[];
  /** Written beside `path`: the video's `.vtt` captions. */
  sidecars: string[];
  /** Cache files deleted afterwards. */
  pruned: string[];
  ms: number;
}

/** Renders `dir` to `<dir>/renders/<preset>.<ext>`. Resolves to the report,
 *  or null when cancelled; rejects with a sentence fit to show when the
 *  render fails or `dir` already has one running. */
export function renderVideo(dir: string, preset: RenderPreset): Promise<RenderReport | null> {
  return invoke<RenderReport | null>("studio_render", { dir, preset });
}

/** Mirrors `render::ExportStatus`: one preset's file and whether a render now
 *  would make the same file. */
export interface ExportStatus {
  preset: RenderPreset;
  /** Absolute path of `renders/<preset>.<ext>`, when it exists. */
  path: string | null;
  /** Seconds since the epoch. */
  modified: number | null;
  current: boolean;
  /** Why the preset cannot render now, a sentence fit to show. */
  problem: string | null;
}

/** Every preset's status, in the menu's order. Renders nothing. */
export function exportStatus(dir: string): Promise<ExportStatus[]> {
  return invoke<ExportStatus[]>("studio_exports", { dir });
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
export type RenderProgress = { dir: string; preset: RenderPreset } & (
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
