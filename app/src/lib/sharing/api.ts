import { invoke } from "@tauri-apps/api/core";
import type { ShareEndpoints, ShareInfo, SharingApi } from "./types";

/** The `companion_*` commands (`app/src-tauri/src/companion/commands.rs`). They
 * reject with `{code, message}`, which `shareFailure` reads as is. */
export const tauriSharingApi: SharingApi = {
  start: (fileId) => invoke<ShareInfo>("companion_share_start", { fileId }),
  stop: (fileId) => invoke<void>("companion_share_stop", { fileId }),
  stopAll: () => invoke<void>("companion_stop_all"),
  shares: () => invoke<ShareInfo[]>("companion_shares"),
  endpoints: () => invoke<ShareEndpoints>("companion_endpoints"),
};

export const COMPANION_STATE_EVENT = "companion-state";
