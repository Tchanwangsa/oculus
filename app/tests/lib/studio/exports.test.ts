import { expect, test } from "bun:test";

import { exportState, playableRenders, presetHint } from "@/lib/studio/exports";
import type { ExportStatus } from "@/lib/studio/render";
import type { RenderFile } from "@/lib/studio/videos";

test("each preset says what it makes for the video's shape", () => {
  expect(presetHint("draft", 1920, 1080)).toBe("mp4 · 540p, quick");
  expect(presetHint("final", 1920, 1080)).toBe("mp4 · 1080p");
  expect(presetHint("small", 1920, 1080)).toBe("mp4 · 720p, smaller file");
  expect(presetHint("2x", 1920, 1080)).toBe("mp4 · 3840×2160");
  expect(presetHint("small", 1080, 1920)).toBe("mp4 · 720×1280, smaller file");
  // A small video is never scaled up.
  expect(presetHint("small", 640, 360)).toBe("mp4 · 360p, smaller file");
  expect(presetHint("subtitles", 1920, 1080)).toBe("vtt and srt");
});

test("a row is current, out of date, never made or blocked", () => {
  const row = (patch: Partial<ExportStatus>): ExportStatus => ({
    preset: "final",
    path: null,
    modified: null,
    current: false,
    problem: null,
    ...patch,
  });
  expect(exportState(row({ path: "/v/renders/final.mp4", current: true }))).toBe("current");
  expect(exportState(row({ path: "/v/renders/final.mp4" }))).toBe("stale");
  expect(exportState(row({}))).toBe("none");
  expect(exportState(row({ path: "/v/renders/final.mp4", problem: "stale narration" }))).toBe("blocked");
});

test("the player plays video renders only, newest first", () => {
  const file = (preset: RenderFile["preset"], modified: number): RenderFile => ({
    preset,
    path: `/v/renders/${preset}`,
    modified,
    captions: false,
  });
  const renders = [file("draft", 1), file("audio", 9), file("small", 5), file("subtitles", 7)];
  expect(playableRenders(renders).map((r) => r.preset)).toEqual(["small", "draft"]);
});
