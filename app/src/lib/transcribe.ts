import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getSetting, setSetting } from "@/lib/db";

/**
 * Video transcription (`app/src-tauri/src/transcribe/`), the Groq key it
 * runs on, and the `transcribe` settings row: the engine order, the one
 * language, and each engine's switch. Rust writes `<video>.vtt` beside the
 * video and records nothing; a caller that tracks transcripts in the DB
 * stores the returned path itself.
 */

/** `ENGINES` in `app/src-tauri/src/transcribe/mod.rs`. */
export type TranscribeEngine = "groq" | "whisper" | "apple";

/** The order a run tries the engines in until Settings changes it. */
export const DEFAULT_ENGINE_ORDER: readonly TranscribeEngine[] = ["groq", "whisper", "apple"];

/** As Rust's `engine_label` names each engine in its messages. */
export const ENGINE_LABELS: Record<TranscribeEngine, string> = {
  groq: "Groq",
  apple: "on-device speech",
  whisper: "local Whisper",
};

/** Mirrors `Progress` in `app/src-tauri/src/transcribe/mod.rs`. */
export interface TranscribeProgress {
  /** The path exactly as passed to `transcribe`. */
  path: string;
  phase: "extracting" | "transcribing" | "complete" | "error";
  /** The engine at work, on `transcribing` and `complete`. */
  engine?: TranscribeEngine;
  /** From 1 while transcribing; 0 while extracting. */
  chunk: number;
  chunks: number;
  /** Only on `error`; the same text the call rejects with. */
  error?: string;
}

/**
 * Transcribe a video inside the library — absolute, or relative to the data
 * directory — and resolve to the absolute path of the `.vtt` written beside
 * it. Rejects with a sentence fit to show: no key, Groq's rate limit and when
 * to retry, or what failed. Nothing is written on failure.
 */
export function transcribe(path: string): Promise<string> {
  return invoke<string>("transcribe_video", { path });
}

export function onTranscribeProgress(
  handler: (progress: TranscribeProgress) => void,
): Promise<UnlistenFn> {
  return listen<TranscribeProgress>("transcribe-progress", (event) => handler(event.payload));
}

export function hasGroqKey(): Promise<boolean> {
  return invoke<boolean>("groq_has_api_key");
}

/**
 * Rust checks the key against Groq's free model list before keeping it.
 * `"unverified"`: Groq was unreachable or rate-limited, and it was kept anyway.
 */
export function saveGroqKey(key: string): Promise<"ok" | "unverified"> {
  return invoke<"ok" | "unverified">("groq_set_api_key", { key });
}

export function deleteGroqKey(): Promise<void> {
  return invoke<void>("groq_delete_api_key");
}

// ── On-device speech ─────────────────────────────────────────────────────────
//
// macOS 26's speech recogniser. Settings only reads its status; downloading a
// language happens at transcription time.

/** Mirrors the `apple_speech_status` command's answer. Locale ids are `en_AU`. */
export interface AppleSpeechStatus {
  available: boolean;
  /** Why it is unavailable, in a sentence fit to show; null when available. */
  reason: string | null;
  supported: string[];
  /** Locales whose model is already on this Mac. */
  installed: string[];
  defaultLocale: string | null;
}

/** Resolves `available: false` with a reason rather than rejecting when the
 *  recogniser is missing; rejects only on an unexpected failure. */
export function appleSpeechStatus(): Promise<AppleSpeechStatus> {
  return invoke<AppleSpeechStatus>("apple_speech_status");
}

// ── Local Whisper ────────────────────────────────────────────────────────────
//
// whisper.cpp on this Mac with a model downloaded from Hugging Face
// (`app/src-tauri/src/transcribe/whisper.rs`). Listing the models is local;
// only a download touches the network.

/** Mirrors `Fit` in `app/src-tauri/src/transcribe/whisper_models.rs`. */
export type WhisperFit = "recommended" | "ok" | "too_large";

export interface WhisperModel {
  id: string;
  label: string;
  /** The file's size. */
  bytes: number;
  /** Roughly what a run holds in memory. */
  ramBytes: number;
  downloaded: boolean;
  /** A download is in flight in the app, perhaps started by an earlier visit. */
  downloading: boolean;
  fit: WhisperFit;
}

/** Mirrors `WhisperListing` (a `Catalogue` and the helper);
 *  `models` runs worst to best transcription. */
export interface WhisperCatalogue {
  /** `whisper-cli` is found; without it no model runs. */
  helper: boolean;
  /** null when the RAM size could not be read. */
  totalRamBytes: number | null;
  gpu: boolean;
  vadDownloaded: boolean;
  /** The model a run uses when none is chosen and it is downloaded. */
  defaultModel: string;
  models: WhisperModel[];
}

export function whisperModels(): Promise<WhisperCatalogue> {
  return invoke<WhisperCatalogue>("whisper_models");
}

/** Resolves once the model is on disk; rejects `"cancelled"` on a cancel,
 *  else with a sentence fit to show. */
export function downloadWhisperModel(id: string): Promise<void> {
  return invoke<void>("whisper_download_model", { id });
}

/** false when no download of `id` is running. */
export function cancelWhisperDownload(id: string): Promise<boolean> {
  return invoke<boolean>("whisper_cancel_download", { id });
}

/** Resolves to the bytes freed. */
export function deleteWhisperModel(id: string): Promise<number> {
  return invoke<number>("whisper_delete_model", { id });
}

/** Mirrors `ModelProgress`; throttled to about one every 150 ms. */
export interface WhisperModelProgress {
  id: string;
  phase: "downloading" | "complete" | "cancelled" | "error";
  /** Bytes of the model file. */
  received: number;
  total: number;
  /** Only on `error`. */
  error?: string;
}

export function onWhisperModelProgress(
  handler: (progress: WhisperModelProgress) => void,
): Promise<UnlistenFn> {
  return listen<WhisperModelProgress>("whisper-model-progress", (event) => handler(event.payload));
}

/**
 * The model a run would use, as Rust's `pick` chooses it: the chosen one when
 * set (null if it is not downloaded — the run would fail), else the default if
 * downloaded, else the last downloaded in catalogue order.
 */
export function pickWhisperModel(
  catalogue: Pick<WhisperCatalogue, "models" | "defaultModel">,
  chosen: string | null,
): WhisperModel | null {
  const onDisk = catalogue.models.filter((m) => m.downloaded);
  if (chosen) return onDisk.find((m) => m.id === chosen) ?? null;
  return onDisk.find((m) => m.id === catalogue.defaultModel) ?? onDisk[onDisk.length - 1] ?? null;
}

// ── The `transcribe` row ─────────────────────────────────────────────────────
//
// Rust reads it at the start of every run (`settings_from` in
// `app/src-tauri/src/transcribe/mod.rs`); the two parsers move together.

export interface TranscribeSettings {
  /** Each engine once, in the order a run tries them. */
  order: TranscribeEngine[];
  /** `auto`, a locale such as `en_AU`, or a bare language code; null is the
   *  default, English in the Mac's region. */
  language: string | null;
  groq: { enabled: boolean };
  whisper: { enabled: boolean; model: string | null };
  apple: { enabled: boolean };
}

const TRANSCRIBE_KEY = "transcribe";

type Json = Record<string, unknown>;

function object(value: unknown): Json {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Json) : {};
}

function text(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

/** Known names in the stored order, each once, then any left out in the
 *  default order. */
function resolveOrder(stored: unknown): TranscribeEngine[] {
  const named = Array.isArray(stored) ? stored : [];
  const order: TranscribeEngine[] = [];
  for (const name of [...named, ...DEFAULT_ENGINE_ORDER]) {
    const engine = DEFAULT_ENGINE_ORDER.find((e) => e === name);
    if (engine && !order.includes(engine)) order.push(engine);
  }
  return order;
}

/**
 * The row as Rust reads it: a missing or mistyped value is its default — the
 * default order, every engine on, the default language, the picked model —
 * and without a top-level `language`, the older `apple.locale`, then
 * `whisper.language`, stands in for it.
 */
export function parseTranscribeSettings(row: Json): TranscribeSettings {
  const groq = object(row.groq);
  const whisper = object(row.whisper);
  const apple = object(row.apple);
  const enabled = (engine: Json) => (typeof engine.enabled === "boolean" ? engine.enabled : true);
  const language = text(row.language) ?? text(apple.locale) ?? text(whisper.language);
  return {
    order: resolveOrder(row.order),
    language: language?.toLowerCase() === "auto" ? "auto" : language,
    groq: { enabled: enabled(groq) },
    whisper: { enabled: enabled(whisper), model: text(whisper.model) },
    apple: { enabled: enabled(apple) },
  };
}

/** The row as stored, unknown keys included, so a write keeps them. */
async function readTranscribeRow(): Promise<Json> {
  const raw = await getSetting(TRANSCRIBE_KEY);
  if (!raw) return {};
  try {
    return object(JSON.parse(raw));
  } catch {
    return {};
  }
}

export async function readTranscribeSettings(): Promise<TranscribeSettings> {
  return parseTranscribeSettings(await readTranscribeRow());
}

export interface TranscribeSettingsPatch {
  order?: TranscribeEngine[];
  language?: string;
  groq?: { enabled: boolean };
  whisper?: { enabled?: boolean; model?: string | null };
  apple?: { enabled: boolean };
}

/**
 * Read-modify-write of the whole row, so keys this side doesn't know survive.
 * A language is written only at the top level, and the older per-engine keys
 * it replaces are dropped with it.
 */
export async function writeTranscribeSettings(
  patch: TranscribeSettingsPatch,
): Promise<TranscribeSettings> {
  const row = await readTranscribeRow();
  const next: Json = { ...row };
  if (patch.order) next.order = resolveOrder(patch.order);
  if (patch.language !== undefined) {
    next.language = patch.language;
    const { locale: _, ...apple } = object(row.apple);
    const { language: __, ...whisper } = object(row.whisper);
    if ("apple" in row) next.apple = apple;
    if ("whisper" in row) next.whisper = whisper;
  }
  for (const engine of ["groq", "whisper", "apple"] as const) {
    const part = patch[engine];
    if (part) next[engine] = { ...object(next[engine]), ...part };
  }
  await setSetting(TRANSCRIBE_KEY, JSON.stringify(next));
  return parseTranscribeSettings(next);
}

// ── Language ─────────────────────────────────────────────────────────────────

const NAMES = new Intl.DisplayNames(["en"], { type: "language", languageDisplay: "standard" });

/** `en_AU` → "English (Australia)", `fr` → "French"; an id Intl can't read
 *  shows as itself. */
export function localeName(id: string): string {
  try {
    return NAMES.of(id.replace(/_/g, "-")) ?? id;
  } catch {
    return id;
  }
}

function languageOf(id: string): string {
  return id.split(/[_-]/)[0].toLowerCase();
}

/** Offered when on-device speech can't list its locales: Whisper's and Groq's
 *  codes, which need no region. */
const FALLBACK_LANGUAGES = ["en", "zh", "es", "fr", "de", "ja", "ko", "hi", "id", "vi", "th", "ar"];

export interface LanguageOption {
  value: string;
  name: string;
}

/**
 * The language select's choices: Auto-detect, then English variants, then the
 * rest by name. On-device speech's locales when it lists them (it is the one
 * engine that needs a region), else bare codes. A region shows only where a
 * language has more than one. `current` is kept as a choice even when the
 * list lacks it.
 */
export function languageOptions(supported: readonly string[], current: string | null): LanguageOption[] {
  const ids = supported.length ? [...supported] : [...FALLBACK_LANGUAGES];
  if (current && current !== "auto" && !ids.includes(current)) ids.push(current);
  const perLanguage = new Map<string, number>();
  for (const id of ids) perLanguage.set(languageOf(id), (perLanguage.get(languageOf(id)) ?? 0) + 1);
  const named = ids.map((id) => ({
    value: id,
    name: (perLanguage.get(languageOf(id)) ?? 0) > 1 ? localeName(id) : localeName(languageOf(id)),
  }));
  const english = (id: string) => languageOf(id) === "en";
  named.sort((a, b) => {
    const rank = Number(english(b.value)) - Number(english(a.value));
    return rank !== 0 ? rank : a.name.localeCompare(b.name);
  });
  return [{ value: "auto", name: "Auto-detect" }, ...named];
}

/**
 * The select's value for the stored language: the default (null, or the old
 * Whisper default `en`) is on-device speech's own default locale, else plain
 * English.
 */
export function languageValue(language: string | null, defaultLocale: string | null): string {
  if (language === null || language.toLowerCase() === "en") return defaultLocale ?? "en";
  return language;
}

/** The locale on-device speech runs in, as `Language::apple` in Rust picks
 *  it: its own default for the default language and for auto-detect. */
export function appleLocale(language: string | null, defaultLocale: string | null): string | null {
  if (language === null || language === "auto" || language.toLowerCase() === "en") return defaultLocale;
  return language;
}
