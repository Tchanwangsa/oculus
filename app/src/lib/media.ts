import { invoke } from "@tauri-apps/api/core";

interface MediaServerInfo {
  port: number;
  token: string;
}

let infoPromise: Promise<MediaServerInfo> | null = null;

/**
 * URL for streaming a local media file through the Rust media HTTP server.
 *
 * WebKit's media pipeline refuses `<video>`/`<audio>` sources on custom URL
 * schemes — `convertFileSrc` URLs fetch fine but fail instantly with
 * MEDIA_ERR_SRC_NOT_SUPPORTED when given to a media element (macOS 26).
 * Real localhost HTTP is the only thing it accepts for local files; the
 * server lives in `app/src-tauri/src/media.rs`.
 */
export async function mediaSrc(absolutePath: string): Promise<string> {
  infoPromise ??= invoke<MediaServerInfo>("media_server_info");
  const { port, token } = await infoPromise;
  return `http://127.0.0.1:${port}/${token}?path=${encodeURIComponent(absolutePath)}`;
}
