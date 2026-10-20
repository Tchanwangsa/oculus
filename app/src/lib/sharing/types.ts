/** What the device-sharing commands return and the `companion-state` event
 * carries (`app/src-tauri/src/companion/wire.rs`, docs/device-sharing.md). */

export type SharePhase = "live" | "paused" | "ended";

/** One way to reach this Mac; `id` (`<interface>/<ip>`) is stable. `url` is the
 * share link inside a `ShareInfo`, the bare page from `companion_endpoints` and
 * the event, and null while nothing listens. `connectedClients` counts devices
 * whose live socket came in through this address (that share's, inside a
 * `ShareInfo`; every share's otherwise); `reachable` is only `connectedClients > 0`. */
export interface ShareEndpoint {
  id: string;
  url: string | null;
  address: string;
  family: "ipv4" | "ipv6";
  interface: string;
  label: string;
  source: string;
  generation: number;
  connectedClients: number;
  reachable: boolean;
}

/** A live share; its endpoints' URLs hold the secret, so it is never logged. */
export interface ShareInfo {
  fileId: number;
  shareId: string;
  epoch: string;
  port: number;
  generation: number;
  endpoints: ShareEndpoint[];
  clients: number;
  phase: SharePhase;
}

/** A share as the event reports it: no link, no secret. */
export interface ShareSummary {
  fileId: number;
  shareId: string;
  epoch: string;
  clients: number;
  phase: SharePhase;
}

export interface CompanionState {
  running: boolean;
  port: number | null;
  ipv6: boolean;
  /** Moves when this Mac's address list changes; the event is sent then too. */
  generation: number;
  /** Bare `/c/` pages, no key; a share's links come from `companion_shares`. */
  endpoints: ShareEndpoint[];
  shares: ShareSummary[];
}

export interface ShareEndpoints {
  port: number | null;
  generation: number;
  endpoints: ShareEndpoint[];
}

/** The sharing commands, injected so the controller runs on fakes in tests. */
export interface SharingApi {
  start(fileId: number): Promise<ShareInfo>;
  stop(fileId: number): Promise<void>;
  stopAll(): Promise<void>;
  shares(): Promise<ShareInfo[]>;
  endpoints(): Promise<ShareEndpoints>;
}

/** The native codes (`companion/error.rs`); an authority code when attaching failed. */
export type SharingErrorCode = "not-live" | "server" | "random";
