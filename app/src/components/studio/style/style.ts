/** The choices the style chip makes for a new video. */
export interface StudioStyle {
  /** A theme picked by hand; null leaves it to the subject's default. */
  theme: string | null;
  format: "video" | "reels";
  /** Target length in seconds. */
  lengthSecs: number;
}

export const FORMATS: ReadonlyArray<{ value: StudioStyle["format"]; label: string }> = [
  { value: "video", label: "Video" },
  { value: "reels", label: "Reels" },
];

export const LENGTHS: ReadonlyArray<{ value: number; label: string }> = [
  { value: 30, label: "30 s" },
  { value: 60, label: "60 s" },
  { value: 120, label: "2 min" },
  { value: 300, label: "5 min" },
];

export const DEFAULT_STYLE: StudioStyle = { theme: null, format: "video", lengthSecs: 60 };

export const lengthLabel = (secs: number) => LENGTHS.find((l) => l.value === secs)?.label ?? `${secs} s`;

/** The stored format and length (the theme is per video, so not kept), read
 *  tolerantly: anything unrecognised falls back to the default. */
export function readStyle(raw: string | null): StudioStyle {
  try {
    const v: unknown = JSON.parse(raw ?? "null");
    if (v && typeof v === "object") {
      const { format, lengthSecs } = v as Record<string, unknown>;
      return {
        theme: null,
        format: format === "reels" ? "reels" : "video",
        lengthSecs: LENGTHS.some((l) => l.value === lengthSecs) ? (lengthSecs as number) : DEFAULT_STYLE.lengthSecs,
      };
    }
  } catch {
    /* fall through to the default */
  }
  return DEFAULT_STYLE;
}

export const writeStyle = (s: StudioStyle) => JSON.stringify({ format: s.format, lengthSecs: s.lengthSecs });
