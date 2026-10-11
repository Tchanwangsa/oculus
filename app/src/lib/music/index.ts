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

/** A track to play after the current one (`up_next`). */
export interface MusicNextTrack {
  uri: string;
  /** Added to the queue, as against next in the context. */
  queued: boolean;
}

/** What another Connect device plays while it holds playback (`remote`). */
export interface MusicRemotePlayback {
  device_name: string;
  /** Null until Spotify's metadata or the library names it. */
  track: MusicTrack | null;
  position_ms: number;
  position_at_ms: number;
  playing: boolean;
  /** 0–100; null when the device takes no volume commands. */
  volume: number | null;
  shuffle: boolean;
  repeat: boolean;
}

export interface MusicSnapshot {
  connected: boolean;
  /** A login, a connect or a reconnect is under way. */
  connecting: boolean;
  /** No stored login, or Spotify refused it: `music.login()` next. */
  needs_login: boolean;
  account: string | null;
  device_name: string;
  /** Oculus is Spotify's active device. While false, the player's commands
   *  go to the device playing elsewhere, or pull playback here when nothing
   *  plays; `music.pause` does nothing. */
  active: boolean;
  /** The active device's name when it is another one, if known. */
  playing_on: string | null;
  /** The chosen output on this Mac; null is the system default. */
  output: string | null;
  /** What the device playing elsewhere plays; null while Oculus is active or
   *  nothing is. The fields below stay Oculus's own (the exclusive rule reads
   *  them); the player shows `shownPlayback`. */
  remote: MusicRemotePlayback | null;
  track: MusicTrack | null;
  /** True at `position_at_ms` (epoch ms); add the time since while playing. */
  position_ms: number;
  position_at_ms: number;
  playing: boolean;
  /** 0–100. */
  volume: number;
  shuffle: boolean;
  repeat: boolean;
  /** What plays next, queued tracks first, every catalogue track Spotify
   *  lists (up to 80 while Oculus plays): from Oculus's own queue while it
   *  plays, from Spotify's device list otherwise. Names come from
   *  `music.tracks`. */
  up_next: MusicNextTrack[];
  /** The playlist, album or other context the device playing plays from;
   *  null when unknown. */
  context_uri: string | null;
  /** The latest failure, kept until the next connect or load. */
  error: string | null;
}

/** One of the account's playlists (`music_playlists`). */
export interface MusicPlaylist {
  uri: string;
  name: string;
  owner: string | null;
  track_count: number | null;
  /** The cover: one URL, or four drawn as a 2×2 grid; empty until one is
   *  known. A playlist Spotify gives no picture gets one from
   *  `music.playlistCovers`. */
  images: string[];
}

/** One track's row (`music_tracks`). */
export interface MusicTrackRow {
  uri: string;
  name: string;
  artists: string[];
  album: string;
  image_url: string | null;
  duration_ms: number;
}

/** One search hit (`music_search`). */
export interface MusicSearchItem {
  uri: string;
  name: string;
  /** The owner, the artists, the artists and album, or "Artist". */
  subtitle: string;
  image_url: string | null;
  /** A track's album, so the track plays on into it. */
  context_uri: string | null;
}

export interface MusicSearchResults {
  tracks: MusicSearchItem[];
  artists: MusicSearchItem[];
  albums: MusicSearchItem[];
  playlists: MusicSearchItem[];
}

/** A search group that pages (`music_search_more`). */
export type MusicSearchKind = "tracks" | "albums" | "playlists";

/** One page of a group; `next_offset` is null on the last. */
export interface MusicSearchPage {
  items: MusicSearchItem[];
  next_offset: number | null;
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

/** What the player shows: the remote device's playback while another device
 *  plays, Oculus's own otherwise. */
export interface MusicPlayback {
  track: MusicTrack | null;
  position_ms: number;
  position_at_ms: number;
  playing: boolean;
  /** Null when the device playing takes no volume commands. */
  volume: number | null;
  shuffle: boolean;
  repeat: boolean;
  /** The other device's name while it plays; null for Oculus. */
  device: string | null;
}

export function shownPlayback(s: MusicSnapshot): MusicPlayback {
  const r = s.remote;
  if (r) return { ...r, device: r.device_name };
  return {
    track: s.track,
    position_ms: s.position_ms,
    position_at_ms: s.position_at_ms,
    playing: s.playing,
    volume: s.volume,
    shuffle: s.shuffle,
    repeat: s.repeat,
    device: null,
  };
}

/** Whether the transport can act: a track here, or another device playing
 *  that a command would control. */
export function musicControllable(s: MusicSnapshot): boolean {
  return (
    s.connected &&
    (s.track !== null || s.remote !== null || (!s.active && s.playing_on !== null))
  );
}

/** The position now, interpolated from the playback's stamp. */
export function positionNow(
  s: Pick<MusicPlayback, "track" | "position_ms" | "position_at_ms" | "playing">,
  now = Date.now(),
): number {
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
  /** The player's play: resumes the device playing elsewhere, else plays
   *  here (pulling playback here when nothing plays). */
  play: () => invoke("music_play"),
  /** This device only: never starts playback, never pulls it, never touches
   *  another device. What the exclusive-playback rule sends. */
  pause: () => invoke("music_pause"),
  /** The player's pause: pauses what the player shows, here or on the
   *  device playing elsewhere. */
  pausePlayer: () => invoke("music_pause_player"),
  next: () => invoke("music_next"),
  prev: () => invoke("music_prev"),
  seek: (positionMs: number) => invoke("music_seek", { positionMs }),
  setVolume: (percent: number) =>
    invoke("music_volume", { percent: Math.round(Math.min(100, Math.max(0, percent))) }),
  setShuffle: (on: boolean) => invoke("music_shuffle", { on }),
  /** On repeats the context; off also clears a repeated track. */
  setRepeat: (on: boolean) => invoke("music_repeat", { on }),
  /** `spotify:playlist:…`, `spotify:album:…`; played on the device playing
   *  elsewhere, else on this one. */
  playContext: (uri: string) => invoke("music_play_context", { uri }),
  /** A `spotify:track:…`, from its place in `contextUri` (a playlist or
   *  album) when given. */
  playTrack: (uri: string, contextUri: string | null = null) =>
    invoke("music_play_track", { uri, contextUri }),
  /** Calls Spotify can rate-limit: Rust keeps the playlists after the first
   *  fetch, and a 429 refuses every call until its wait is over. */
  playlists: (refresh = false) => invoke<MusicPlaylist[]>("music_playlists", { refresh }),
  /** Covers for playlists without one, made from their first tracks' albums
   *  and kept until a refresh. A URI missing from the answer was cut short
   *  (a rate limit) and may be asked for again. */
  playlistCovers: (uris: string[]) =>
    invoke<Record<string, string[]>>("music_playlist_covers", { uris }),
  /** Every catalogue track of a playlist as URIs, kept until a refresh. */
  playlistTracks: (uri: string) => invoke<string[]>("music_playlist_tracks", { uri }),
  albumTracks: (uri: string) => invoke<string[]>("music_album_tracks", { uri }),
  /** Rows by URI, kept once fetched; ask for a page of ~100 at a time. */
  tracks: (uris: string[]) => invoke<Record<string, MusicTrackRow>>("music_tracks", { uris }),
  /** To the queue of the device playing, this one or another; refused
   *  while nothing plays. */
  addToQueue: (uri: string) => invoke("music_add_to_queue", { uri }),
  /** Plays the `up_next` entry at `index` (named `uri`) now, on the device
   *  playing; refused once it is no longer up next. */
  skipTo: (uri: string, index: number) => invoke("music_skip_to", { uri, index }),
  /** Takes the `up_next` entry at `index` (named `uri`) out of the queue of
   *  the device playing. */
  removeFromQueue: (uri: string, index: number) =>
    invoke("music_queue_remove", { uri, index }),
  search: (query: string) => invoke<MusicSearchResults>("music_search", { query }),
  searchMore: (query: string, kind: MusicSearchKind, offset: number) =>
    invoke<MusicSearchPage>("music_search_more", { query, kind, offset }),
  /** The account's Connect devices from Spotify's last pushed list; only
   *  this device until the first push. Costs no call. */
  devices: () => invoke<MusicDevice[]>("music_devices"),
  /** Moves playback to the device and plays; this device's id pulls it here. */
  transfer: (deviceId: string) => invoke("music_transfer", { deviceId }),
  /** Moves playback from the device playing elsewhere to this one. */
  playHere: () => invoke("music_play_here"),
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

/** Track rows fetched per `music.tracks` call. */
export const TRACK_PAGE = 100;

/** "artists · album" for a track row. */
export function trackSubtitle(row: MusicTrackRow): string {
  return [row.artists.join(", "), row.album].filter(Boolean).join(" · ");
}

/** The `TRACK_PAGE` pages holding rows `first` to `last` (inclusive). */
export function trackPages(first: number, last: number): number[] {
  if (last < first || first < 0) return [];
  const pages: number[] = [];
  for (let p = Math.floor(first / TRACK_PAGE); p <= Math.floor(last / TRACK_PAGE); p++) {
    pages.push(p);
  }
  return pages;
}

/** "owner · N tracks" for a playlist. */
export function playlistSubtitle(p: MusicPlaylist): string {
  const count =
    p.track_count === null ? null : `${p.track_count} ${p.track_count === 1 ? "track" : "tracks"}`;
  return [p.owner, count].filter(Boolean).join(" · ");
}
