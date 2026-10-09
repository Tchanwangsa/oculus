import type { EngineOption } from "../../shared/EngineSelect";

/** Mirrors `ParseSettings` in `app/src-tauri/src/parse/commands.rs`. */
export interface ParseSettings {
  engine: string;
  /** The endpoint in force — the override when there is one, else the default. */
  base_url: string;
  /** What the endpoint field offers when nothing is overridden. */
  default_base_url: string;
  overridden: boolean;
  /** The version *this app* writes, for the handshake below. */
  parser_version: number;
  credentials_ready: boolean;
  /** Download results through MinerU's expired CDN certificate. */
  accept_expired_result_cert: boolean;
  engines: EngineOption[];
}

/** Mirrors `LocalProbe` in `app/src-tauri/src/parse/commands.rs`. */
export interface LocalProbe {
  state: "reachable" | "unreachable" | "version_mismatch";
  base_url: string;
  backend: string | null;
  parser_version: number | null;
  /** Always present when `state` is not `reachable`. */
  detail: string | null;
}
