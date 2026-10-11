import { invoke } from "@tauri-apps/api/core";
import { loadDataDir } from "@/hooks/backend/useDataDir";

interface MediaServerInfo {
  port: number;
  token: string;
}

let infoPromise: Promise<MediaServerInfo> | null = null;

/**
 * URL for a library video via the localhost server in `media.rs`, from an
 * absolute path or one relative to the data dir. WebKit (macOS 26) rejects
 * media on custom schemes like `convertFileSrc`'s with
 * MEDIA_ERR_SRC_NOT_SUPPORTED.
 */
export async function mediaSrc(path: string): Promise<string> {
  infoPromise ??= invoke<MediaServerInfo>("media_server_info");
  const absolute = path.startsWith("/") ? path : `${await loadDataDir()}/${path}`;
  const { port, token } = await infoPromise;
  return `http://127.0.0.1:${port}/${token}?path=${encodeURIComponent(absolute)}`;
}
