import { invoke } from "@tauri-apps/api/core";

import type { RenderPreset } from "@/lib/studio/render";

// Studio's videos under `<data>/studio/videos/<slug>/`
// (`app/src-tauri/src/studio/commands/videos.rs`). A slug is `[a-z0-9-]+`.

/** Mirrors `videos::VideoRow` plus the thread link. */
export interface VideoRow {
  slug: string;
  /** Absolute folder. */
  dir: string;
  title: string;
  subject: string | null;
  /** null when video.json does not load; `problem` says why. */
  scenes: number | null;
  /** A narration manifest exists; it may be stale. */
  narrated: boolean;
  /** File names in `renders/`: `draft.mp4`, `audio.m4a`, … */
  renders: string[];
  /** Seconds since the epoch. */
  modified: number | null;
  problem: string | null;
  /** The harness thread that made it, if any. */
  threadId: number | null;
}

/** Videos newest first; `subject` is a course code. */
export function listVideos(subject?: string | null): Promise<VideoRow[]> {
  return invoke<VideoRow[]>("studio_videos", { subject: subject ?? null });
}

export interface NewVideoOptions {
  title?: string;
  subject?: string | null;
  /** Defaults to the subject's default theme, else `default`. */
  theme?: string;
  /** 1080x1920 instead of 1920x1080. */
  portrait?: boolean;
}

/** Scaffolds a video; the slug comes from the title and is unique. */
export function newVideo(options: NewVideoOptions): Promise<{ slug: string; dir: string }> {
  return invoke("studio_new_video", { options });
}

/** One scene of a video, in order. Times are seconds and null until the
 *  video is narrated. */
export interface SceneInfo {
  id: string | null;
  component: string;
  start: number | null;
  duration: number | null;
  /** The scene's narration, from `script.md`'s `## id` section. */
  text: string | null;
}

/** One export preset's file. */
export interface RenderFile {
  preset: RenderPreset;
  /** Absolute path. */
  path: string;
  /** Seconds since the epoch. */
  modified: number;
  /** A video render's captions are beside it as `<preset>.vtt`. */
  captions: boolean;
}

export interface VideoDetail {
  row: VideoRow;
  fps: number;
  width: number;
  height: number;
  theme: string;
  /** The narration voice id, from video.json or Settings -> Voice; null if
   *  neither names one. */
  voice: string | null;
  /** video.json's `captions`: the renders have captions burned in. */
  captions: boolean;
  scenes: SceneInfo[];
  renders: RenderFile[];
}

export function videoDetail(slug: string): Promise<VideoDetail> {
  return invoke<VideoDetail>("studio_video_detail", { slug });
}

/** Turns the video's burned-in captions on or off (video.json's `captions`). */
export function setCaptions(slug: string, on: boolean): Promise<void> {
  return invoke("studio_set_captions", { slug, on });
}

/** The WebVTT text beside the video's `preset` render. */
export function renderCaptions(slug: string, preset: RenderPreset): Promise<string> {
  return invoke<string>("studio_render_captions", { slug, preset });
}

/** Where to take a still: `t` seconds into scene `scene`, or `global`
 *  seconds into the whole video. */
export type FrameAt = { scene: number; t: number } | { global: number };

/** A PNG `width` pixels wide. Rejects with a sentence fit to show, for
 *  instance when the narration is stale. */
export async function studioFrame(slug: string, at: FrameAt, width: number): Promise<Blob> {
  const bytes = await invoke<ArrayBuffer>("studio_frame", { slug, at, width });
  return new Blob([bytes], { type: "image/png" });
}

/** The video's cached thumbnail PNG. */
export async function studioThumbnail(slug: string): Promise<Blob> {
  const bytes = await invoke<ArrayBuffer>("studio_thumbnail", { slug });
  return new Blob([bytes], { type: "image/png" });
}

/** What a studio thread carries with its first message; mirrors
 *  `SendOptions.studio` in `lib/harness/commands.ts`. */
export interface StudioSend {
  slug: string;
  /** Target length in seconds. */
  lengthSecs?: number;
  format?: "video" | "reels";
}
