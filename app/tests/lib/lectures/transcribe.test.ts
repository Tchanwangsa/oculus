import { afterAll, expect, mock, spyOn, test } from "bun:test";
import * as db from "@/lib/db";
import type { TranscribeEngine } from "@/lib/lectures/transcribe";

mock.module("@tauri-apps/api/core", () => ({ invoke: async () => null }));
mock.module("@tauri-apps/api/event", () => ({ listen: async () => () => {} }));
let stored: string | null = null;
const read = spyOn(db, "getSetting").mockImplementation(async () => stored);
const write = spyOn(db, "setSetting").mockImplementation(async (_, value) => {
  stored = value;
});
afterAll(() => {
  read.mockRestore();
  write.mockRestore();
});

const {
  appleLocale,
  languageOptions,
  languageValue,
  pickWhisperModel,
  readTranscribeSettings,
  writeTranscribeSettings,
} = await import("@/lib/lectures/transcribe");

const read_ = async (row: unknown) => {
  stored = row === null ? null : typeof row === "string" ? row : JSON.stringify(row);
  return readTranscribeSettings();
};

// Mirrors `settings_from` in app/src-tauri/src/transcribe/settings/mod.rs.
test("a missing or mistyped row reads as the default order, every engine on, the default language", async () => {
  const defaults = {
    order: ["groq", "whisper", "apple"] as TranscribeEngine[],
    language: null,
    groq: { enabled: true },
    whisper: { enabled: true, model: null },
    apple: { enabled: true },
  };
  expect(await read_(null)).toEqual(defaults);
  expect(await read_("not json")).toEqual(defaults);
  expect(await read_({ order: "apple", whisper: { enabled: "no", model: "  " }, apple: [1] })).toEqual(defaults);
  expect(
    await read_({ groq: { enabled: false }, whisper: { enabled: false, model: " small " }, apple: { enabled: false } }),
  ).toEqual({
    ...defaults,
    groq: { enabled: false },
    whisper: { enabled: false, model: "small" },
    apple: { enabled: false },
  });
});

test("the order keeps each known engine once and appends the missing in the default order", async () => {
  expect((await read_({ order: ["apple", "groq", "whisper"] })).order).toEqual(["apple", "groq", "whisper"]);
  expect((await read_({ order: ["vosk", 3, "apple", null] })).order).toEqual(["apple", "groq", "whisper"]);
  expect((await read_({ order: ["whisper", "whisper", "apple", "whisper"] })).order).toEqual([
    "whisper",
    "apple",
    "groq",
  ]);
});

test("without a top-level language the older per-engine keys stand in, Apple's first", async () => {
  expect((await read_({ language: "th_TH", apple: { locale: "en_AU" } })).language).toBe("th_TH");
  expect((await read_({ apple: { locale: "en_AU" }, whisper: { language: "fr" } })).language).toBe("en_AU");
  expect((await read_({ apple: { locale: " " }, whisper: { language: "fr" } })).language).toBe("fr");
  expect((await read_({ whisper: { language: "AUTO" } })).language).toBe("auto");
  expect((await read_({ language: 7, whisper: { language: "de" } })).language).toBe("de");
});

test("writing keeps keys this side doesn't know, and a language replaces the per-engine ones", async () => {
  stored = JSON.stringify({
    apple: { enabled: false, locale: "en_AU" },
    future: 1,
    whisper: { extra: true, language: "en", model: "tiny" },
  });
  await writeTranscribeSettings({ whisper: { model: null } });
  expect(JSON.parse(stored!)).toEqual({
    apple: { enabled: false, locale: "en_AU" },
    future: 1,
    whisper: { extra: true, language: "en", model: null },
  });
  const next = await writeTranscribeSettings({ language: "th_TH", order: ["apple", "groq", "whisper"] });
  expect(JSON.parse(stored!)).toEqual({
    apple: { enabled: false },
    future: 1,
    whisper: { extra: true, model: null },
    language: "th_TH",
    order: ["apple", "groq", "whisper"],
  });
  expect(next.language).toBe("th_TH");
  await writeTranscribeSettings({ groq: { enabled: false } });
  expect(JSON.parse(stored!).groq).toEqual({ enabled: false });
});

// Mirrors `Language::apple` in app/src-tauri/src/transcribe/settings/mod.rs.
test("on-device speech runs in its own default for the default language and for auto-detect", () => {
  expect(appleLocale(null, "en_AU")).toBe("en_AU");
  expect(appleLocale("auto", "en_AU")).toBe("en_AU");
  expect(appleLocale("en", "en_AU")).toBe("en_AU");
  expect(appleLocale("th_TH", "en_AU")).toBe("th_TH");
  expect(languageValue(null, "en_AU")).toBe("en_AU");
  expect(languageValue(null, null)).toBe("en");
  expect(languageValue("auto", "en_AU")).toBe("auto");
});

test("the language list puts English first and shows a region only where a language has several", () => {
  const options = languageOptions(["th_TH", "en_GB", "fr_FR", "en_AU", "fr_CA", "de_DE"], "en_AU");
  expect(options.map((o) => o.name)).toEqual([
    "Auto-detect",
    "English (Australia)",
    "English (UK)",
    "French (Canada)",
    "French (France)",
    "German",
    "Thai",
  ]);
  expect(options.find((o) => o.name === "Thai")?.value).toBe("th_TH");
  // Without on-device speech: bare codes, and the stored value kept as a choice.
  const bare = languageOptions([], "pt");
  expect(bare[1]).toEqual({ value: "en", name: "English" });
  expect(bare.some((o) => o.value === "pt" && o.name === "Portuguese")).toBe(true);
});

// Mirrors `pick` in app/src-tauri/src/transcribe/engines/whisper_models/models.rs.
test("a run uses the chosen model, else the default, else the best on disk", () => {
  const ids = ["tiny", "base", "small", "medium", "large-v3-turbo-q5_0", "large-v3-turbo", "large-v3"];
  const catalogue = (downloaded: string[]) => ({
    defaultModel: "large-v3-turbo-q5_0",
    models: ids.map((id) => ({
      id,
      label: id,
      bytes: 1,
      ramBytes: 1,
      downloaded: downloaded.includes(id),
      downloading: false,
      fit: "ok" as const,
    })),
  });
  expect(pickWhisperModel(catalogue([]), null)).toBeNull();
  expect(pickWhisperModel(catalogue(["tiny", "small"]), null)?.id).toBe("small");
  expect(pickWhisperModel(catalogue(["tiny", "large-v3-turbo-q5_0", "large-v3"]), null)?.id).toBe(
    "large-v3-turbo-q5_0",
  );
  expect(pickWhisperModel(catalogue(["tiny", "small"]), "tiny")?.id).toBe("tiny");
  expect(pickWhisperModel(catalogue(["tiny"]), "medium")).toBeNull();
});
