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
