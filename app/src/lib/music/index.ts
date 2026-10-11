import { invoke } from "@tauri-apps/api/core";

/**
 * Spotify, played by Rust (`app/src-tauri/src/music/`) as a Connect device.
 * Rust owns the state and sends it whole as `MUSIC_STATE_EVENT` on every
 * change; `music.state()` is the mount-time read. Commands return once the
 * work is handed off, and their outcome arrives as the next snapshot.
 */

export const MUSIC_STATE_EVENT = "music-state";
/** The `settings` row Rust reads at startup (`music/startup.rs`):
 *  `{"enabled": boolean}`, off when absent. */
export const MUSIC_SETTING = "music";

export interface MusicTrack {
  uri: string;
  name: string;
  artists: string[];
  album: string;
  /** `https://i.scdn.co/image/<id>`, or null when Spotify lists no cover. */
  cover_url: string | null;
  duration_ms: number;
}

export interface MusicSnapshot {
  connected: boolean;
  /** A login, a connect or a reconnect is under way. */
  connecting: boolean;
  /** No stored login, or Spotify refused it: `music.login()` next. */
  needs_login: boolean;
  account: string | null;
  device_name: string;
  track: MusicTrack | null;
  /** True at `position_at_ms` (epoch ms); add the time since while playing. */
  position_ms: number;
  position_at_ms: number;
  playing: boolean;
  /** 0–100. */
  volume: number;
  shuffle: boolean;
  repeat: boolean;
  /** The latest failure, kept until the next connect or load. */
  error: string | null;
}

/** The settings row's `enabled`; anything unreadable is off. */
export function musicEnabled(raw: string | null): boolean {
  if (!raw) return false;
  try {
    return (JSON.parse(raw) as { enabled?: unknown })?.enabled === true;
  } catch {
    return false;
  }
}

/** The position now, interpolated from the snapshot's stamp. */
export function positionNow(s: MusicSnapshot, now = Date.now()): number {
  if (!s.playing) return s.position_ms;
  const at = s.position_ms + Math.max(0, now - s.position_at_ms);
  return s.track ? Math.min(at, s.track.duration_ms) : at;
}

export const music = {
  state: () => invoke<MusicSnapshot>("music_state"),
  /** Opens Spotify's login in the browser; asking again reopens it. */
  login: () => invoke("music_login"),
  /** From the stored login; `needs_login` when there is none. */
  connect: () => invoke("music_connect"),
  /** Keeps the stored login. */
  disconnect: () => invoke("music_disconnect"),
  /** Also deletes the stored login. */
  signOut: () => invoke("music_sign_out"),
  playPause: () => invoke("music_play_pause"),
  play: () => invoke("music_play"),
  /** Never starts playback, unlike `playPause`. */
  pause: () => invoke("music_pause"),
  next: () => invoke("music_next"),
  prev: () => invoke("music_prev"),
  seek: (positionMs: number) => invoke("music_seek", { positionMs }),
  setVolume: (percent: number) =>
    invoke("music_volume", { percent: Math.round(Math.min(100, Math.max(0, percent))) }),
  setShuffle: (on: boolean) => invoke("music_shuffle", { on }),
  /** On repeats the context; off also clears a repeated track. */
  setRepeat: (on: boolean) => invoke("music_repeat", { on }),
  /** `spotify:playlist:…`, `spotify:album:…`; played on this device. */
  playContext: (uri: string) => invoke("music_play_context", { uri }),
};
