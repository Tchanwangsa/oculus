import { invoke } from "@tauri-apps/api/core";

interface MediaServerInfo {
  port: number;
  token: string;
}

let infoPromise: Promise<MediaServerInfo> | null = null;

/**
 * URL for a local media file via the localhost server in `media.rs`. WebKit
 * (macOS 26) rejects media on custom schemes like `convertFileSrc`'s with
 * MEDIA_ERR_SRC_NOT_SUPPORTED.
 */
export async function mediaSrc(absolutePath: string): Promise<string> {
  infoPromise ??= invoke<MediaServerInfo>("media_server_info");
  const { port, token } = await infoPromise;
  return `http://127.0.0.1:${port}/${token}?path=${encodeURIComponent(absolutePath)}`;
}
