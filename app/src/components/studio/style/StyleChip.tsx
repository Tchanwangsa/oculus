import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { CaretDown, Microphone } from "@phosphor-icons/react";

import { Segmented } from "@/components/studio/style/Segmented";
import { FORMATS, LENGTHS, lengthLabel, type StudioStyle } from "@/components/studio/style/style";
import { Swatch } from "@/components/studio/style/Swatch";
import { ThemeList } from "@/components/studio/style/ThemeList";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import type { ThemesState } from "@/hooks/studio/useThemes";
import { readVoiceSettings } from "@/lib/studio/voice";

const VOICE_SETTINGS = "/settings/voice";

/**
 * The chip beside the subject's: the theme (a swatch and its name, "Default"
 * when none is picked), and the format and length. Its popover also shows the
 * narration voice, read-only: it comes from Settings → Voice.
 */
export function StyleChip({
  style,
  onChange,
  themes,
}: {
  style: StudioStyle;
  onChange: (next: StudioStyle) => void;
  themes: ThemesState;
}) {
  const picked = themes.themes.find((t) => t.name === style.theme) ?? null;
  const label = style.theme ?? "Default";
  const set = (patch: Partial<StudioStyle>) => onChange({ ...style, ...patch });

  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label="Video style"
          className="flex h-6 cursor-pointer items-center gap-1.5 rounded-full border border-border/70 bg-card px-2 text-[11px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          {picked && <Swatch colors={picked.swatch} className="h-3 w-5" />}
          <span className="max-w-[110px] truncate">{label}</span>
          <span aria-hidden className="h-3 w-px bg-border" />
          <span className="whitespace-nowrap">
            {style.format === "reels" ? "Reels" : "Video"} · {lengthLabel(style.lengthSecs)}
          </span>
          <CaretDown size={10} className="shrink-0" />
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" side="top" className="w-72 space-y-3 p-3">
        <Field label="Theme">
          <ThemeList state={themes} value={style.theme} onChange={(theme) => set({ theme })} />
        </Field>
        <Field label="Format">
          <Segmented label="Format" options={FORMATS} value={style.format} onChange={(format) => set({ format })} />
        </Field>
        <Field label="Length">
          <Segmented label="Length" options={LENGTHS} value={style.lengthSecs} onChange={(lengthSecs) => set({ lengthSecs })} />
        </Field>
        <VoiceLine />
      </PopoverContent>
    </Popover>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="space-y-1">
      <div className="px-0.5 text-[11px] font-medium tracking-wide text-muted-foreground">{label}</div>
      {children}
    </div>
  );
}

/** The narration voice from Settings → Voice, with the way there. */
function VoiceLine() {
  const navigate = useNavigate();
  const [voice, setVoice] = useState<string | null | undefined>(undefined);
  useEffect(() => {
    let stale = false;
    readVoiceSettings()
      .then((v) => !stale && setVoice(v.voice))
      .catch(() => !stale && setVoice(null));
    return () => {
      stale = true;
    };
  }, []);
  return (
    <div className="flex items-center gap-2 border-t border-border-subtle px-0.5 pt-2.5 text-[12px]">
      <Microphone size={13} className="shrink-0 text-muted-foreground" />
      <span className="min-w-0 flex-1 truncate text-muted-foreground">
        Voice{" "}
        <span className="text-foreground">{voice === undefined ? "" : (voice ?? "not set")}</span>
      </span>
      <button
        type="button"
        data-tab-href={VOICE_SETTINGS}
        onClick={() => navigate(VOICE_SETTINGS)}
        className="shrink-0 cursor-pointer text-brand underline-offset-2 hover:underline"
      >
        Settings
      </button>
    </div>
  );
}
