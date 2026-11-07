import { mediaServerInfo } from "@/lib/lectures/media/src";

let base: Promise<string> | null = null;
let known: string | null = null;

/** The kit's URL on the media server (`media.rs` serves `dist/widget-kit/`
 *  there, without the token and with CORS `*`), asked once per launch. */
export function widgetKitBase(): Promise<string> {
  base ??= mediaServerInfo().then((info) => (known = `http://127.0.0.1:${info.port}/widget-kit/`));
  return base;
}

/** The kit's URL once known, so a remounted widget builds without waiting. */
export const knownKitBase = () => known;
