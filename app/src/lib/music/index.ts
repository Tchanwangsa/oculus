import { invoke } from "@tauri-apps/api/core";

/**
 * Spotify, played by Rust (`app/src-tauri/src/music/`) as a Connect device.
 * Rust owns the state and sends it whole as `MUSIC_STATE_EVENT` on every
 * change; `music.state()` is the mount-time read. Commands return once the
 * work is handed off, and their outcome arrives as the next snapshot.
 */

export const MUSIC_STATE_EVENT = "music-state";
/** The `settings` row Rust reads at startup (`music/startup.rs`):
 *  `{"enabled": boolean, "output": string | null}`, off when absent. */
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
  /** Oculus is Spotify's active device. While false, commands pull playback
   *  here first, and a pause does nothing. */
  active: boolean;
  /** The active device's name when it is another one, if known. */
  playing_on: string | null;
  /** The chosen output on this Mac; null is the system default. */
  output: string | null;
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

/** One of the account's playlists (`music_playlists`). */
export interface MusicPlaylist {
  uri: string;
  name: string;
  owner: string | null;
  track_count: number | null;
  image_url: string | null;
}

/** One search hit (`music_search`). */
export interface MusicSearchItem {
  uri: string;
  name: string;
  /** The owner, the artists, or the artists and album. */
  subtitle: string;
  image_url: string | null;
  /** A track's album, so the track plays on into it. */
  context_uri: string | null;
}

export interface MusicSearchResults {
  playlists: MusicSearchItem[];
  albums: MusicSearchItem[];
  tracks: MusicSearchItem[];
}

/** One of the account's Spotify Connect devices (`music_devices`). */
export interface MusicDevice {
  id: string;
  name: string;
  kind: "computer" | "phone" | "tablet" | "speaker" | "tv" | "car" | "other";
  active: boolean;
  /** 0–100, when the device reports one. */
  volume: number | null;
  /** This Oculus. */
  is_this: boolean;
}

/** One output device on this Mac (`music_outputs`). */
export interface MusicOutput {
  name: string;
  /** The Mac's default output right now. */
  is_default: boolean;
}

/** The settings row as an object, every key kept so a write never drops one
 *  it does not know; anything unreadable is empty. */
export function musicRow(raw: string | null): Record<string, unknown> {
  if (!raw) return {};
  try {
    const parsed: unknown = JSON.parse(raw);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

/** The settings row's `enabled`; anything unreadable is off. */
export function musicEnabled(raw: string | null): boolean {
  return musicRow(raw).enabled === true;
}

/** Whether the transport can act: a track here, or another device playing
 *  that a command would pull back. */
export function musicControllable(s: MusicSnapshot): boolean {
  return s.connected && (s.track !== null || (!s.active && s.playing_on !== null));
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
  /** A `spotify:track:…`, from its place in `contextUri` when given. */
  playTrack: (uri: string, contextUri: string | null = null) =>
    invoke("music_play_track", { uri, contextUri }),
  /** Web API calls, rate-limited by Spotify: Rust keeps the playlists after
   *  the first fetch, and a 429 refuses every call until its wait is over. */
  playlists: (refresh = false) => invoke<MusicPlaylist[]>("music_playlists", { refresh }),
  search: (query: string) => invoke<MusicSearchResults>("music_search", { query }),
  /** The account's Connect devices: Spotify's pushed list, or the Web API's
   *  on `refresh` (rate-limited) or before the first push. */
  devices: (refresh = false) => invoke<MusicDevice[]>("music_devices", { refresh }),
  /** Moves playback to the device and plays; this device's id pulls it here. */
  transfer: (deviceId: string) => invoke("music_transfer", { deviceId }),
  outputs: () => invoke<MusicOutput[]>("music_outputs"),
  /** null is the system default. Rust only; `musicPrefsStore.setOutput` also
   *  keeps it in the settings row. */
  setOutput: (name: string | null) => invoke("music_set_output", { name }),
  /** The focus-mode seam: lower the volume to `level`% of what it was before
   *  the first duck. A volume the user sets meanwhile wins. */
  duck: (level: number) =>
    invoke("music_duck", { level: Math.round(Math.min(100, Math.max(0, level))) }),
  /** Put the pre-duck volume back; nothing without a held duck. */
  unduck: () => invoke("music_duck", { level: null }),
};
