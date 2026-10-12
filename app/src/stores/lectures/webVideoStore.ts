import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

/**
 * YouTube and Vimeo videos embedded in Canvas pages (`<web-video>`): each
 * one's oEmbed metadata, which `web_video_info` fetches in Rust
 * (`sources/oembed.rs`). Asked once per video per app session; a rejection
 * is dropped so the next mount asks again.
 */

export type WebVideoProvider = "youtube" | "vimeo";

/** `web_video_info`'s answer; every field may be missing. */
export interface WebVideoMeta {
  title: string | null;
  author_name: string | null;
  thumbnail_url: string | null;
  /** Vimeo only. */
  duration_s: number | null;
}

export type WebVideoInfo =
  | { status: "loading" }
  | { status: "ready"; meta: WebVideoMeta }
  | { status: "error"; error: string };

export const useWebVideos = create<{ infos: Record<string, WebVideoInfo> }>(() => ({ infos: {} }));

export const webVideoKey = (provider: WebVideoProvider, videoId: string) => `${provider}:${videoId}`;

const asked = new Map<string, Promise<WebVideoMeta>>();

function put(key: string, info: WebVideoInfo) {
  useWebVideos.setState((s) => ({ infos: { ...s.infos, [key]: info } }));
}

export function webVideoInfo(provider: WebVideoProvider, videoId: string, hash?: string): Promise<WebVideoMeta> {
  const key = webVideoKey(provider, videoId);
  const held = asked.get(key);
  if (held) return held;
  const ask = invoke<WebVideoMeta>("web_video_info", { provider, videoId, hash: hash ?? null });
  asked.set(key, ask);
  put(key, { status: "loading" });
  ask.then(
    (meta) => put(key, { status: "ready", meta }),
    (e) => {
      asked.delete(key);
      put(key, { status: "error", error: String(e) });
    },
  );
  return ask;
}

/** Where Play opens the video: the provider's own page, with an unlisted
 *  Vimeo video's hash. */
export function webVideoUrl(provider: WebVideoProvider, videoId: string, hash?: string): string {
  const id = encodeURIComponent(videoId);
  if (provider === "youtube") return `https://www.youtube.com/watch?v=${id}`;
  return hash ? `https://vimeo.com/${id}/${encodeURIComponent(hash)}` : `https://vimeo.com/${id}`;
}
