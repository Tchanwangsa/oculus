import type { ExportStatus, RenderPreset } from "@/lib/studio/render";
import type { RenderFile } from "@/lib/studio/videos";

// The export presets as the video page shows them: names, what each makes,
// and which renders the player can play.

/** The presets that make an mp4, which the player can show. */
export const VIDEO_PRESETS: readonly RenderPreset[] = ["draft", "final", "small", "2x"];

export const PRESET_LABEL: Record<RenderPreset, string> = {
  draft: "Draft",
  final: "Final",
  small: "Small",
  "2x": "2×",
  audio: "Audio",
  subtitles: "Subtitles",
};

/** What a preset makes, for `width` x `height` video. */
export function presetHint(preset: RenderPreset, width: number, height: number): string {
  const portrait = height > width;
  const sized = (short: number) => {
    const scale = Math.min(1, short / Math.min(width, height));
    const w = Math.round(width * scale);
    const h = Math.round(height * scale);
    return portrait ? `${w}×${h}` : `${h}p`;
  };
  switch (preset) {
    case "draft":
      return `mp4 · ${sized(540)}, quick`;
    case "final":
      return `mp4 · ${portrait ? `${width}×${height}` : `${height}p`}`;
    case "small":
      return `mp4 · ${sized(720)}, smaller file`;
    case "2x":
      return `mp4 · ${width * 2}×${height * 2}`;
    case "audio":
      return "m4a · the narration";
    case "subtitles":
      return "vtt and srt";
  }
}

/** A row's state: `current` (its file matches the video), `stale` (a file
 *  from an older version), `none` (never made), or `blocked` (it cannot be
 *  made now; `problem` says why). */
export type ExportState = "current" | "stale" | "none" | "blocked";

export function exportState(status: ExportStatus): ExportState {
  if (status.current) return "current";
  if (status.problem) return "blocked";
  return status.path ? "stale" : "none";
}

/** The video renders the player can show, newest first. */
export function playableRenders(renders: RenderFile[]): RenderFile[] {
  return renders.filter((r) => VIDEO_PRESETS.includes(r.preset)).sort((a, b) => b.modified - a.modified);
}
