import { invoke } from "@tauri-apps/api/core";

/** Mirrors `CertState` in `app/src-tauri/src/parse/mineru/result_tls.rs`. */
export interface ResultCertState {
  certificate: "unknown" | "valid" | "expired";
  /** Unix seconds, while `certificate` is `"expired"`. */
  expired_at: number | null;
  /** Expired and allowed: results are downloading through the exception. */
  bypassing: boolean;
}

/** `probe` does a bare TLS handshake with MinerU's result CDN first — no API
 *  call, nothing billed. Without it, what the last download found. */
export function resultCertState(probe = false): Promise<ResultCertState> {
  return invoke<ResultCertState>("parse_result_cert", { probe });
}

export function expiryDate(state: ResultCertState): string {
  return state.expired_at == null
    ? "recently"
    : `on ${new Date(state.expired_at * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" })}`;
}
