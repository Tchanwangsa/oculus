import { invoke } from "@tauri-apps/api/core";

import type { SourceNum } from "@/lib/db";

export interface MomentFrame {
  source: SourceNum;
  /** Relative to `agents/`, the thread's cwd. */
  path: string;
}

/**
 * One JPEG per *downloaded* stream at `seconds`, for a dock message — every
 * source, not just the visible one, since either may carry the teaching.
 * Returns `agents/`-relative paths for the agent, not the webview.
 */
export function lectureGrabFrames(lectureId: string, seconds: number): Promise<MomentFrame[]> {
  return invoke<MomentFrame[]>("lecture_grab_frames", {
    lectureId,
    seconds: Math.max(0, Math.floor(seconds)),
  });
}
