import { useNavigate } from "react-router-dom";
import { CaretDown, Microphone } from "@phosphor-icons/react";

import { Segmented } from "@/components/studio/style/Segmented";
import { LengthField } from "@/components/studio/style/LengthField";
import { FORMATS, lengthLabel, type StudioStyle } from "@/components/studio/style/style";
import { Swatch } from "@/components/studio/style/Swatch";
import { ThemeList } from "@/components/studio/style/ThemeList";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import type { ThemesState } from "@/hooks/studio/useThemes";
import { useSetupProblems } from "@/hooks/studio/useSetupProblems";
import { NARRATION_SETTINGS_HREF } from "@/lib/studio/voice";

/**
 * The chip beside the subject's: the theme (a swatch and its name: the one
 * picked by hand, else the subject's default), and the format and length
 * (Auto, a preset or a typed one). Its popover also shows the narration voice,
 * read-only: it comes from Settings → Studio → Narration, and reads as a warning while a
 * video could not be narrated with it.
 */
export function StyleChip({
  style,
  onChange,
  themes,
  defaultTheme,
}: {
  style: StudioStyle;
  onChange: (next: StudioStyle) => void;
  themes: ThemesState;
  /** The subject's default theme, once known. */
  defaultTheme: string | null;
}) {
  const shown = style.theme ?? defaultTheme;
  const picked = themes.themes.find((t) => t.name === shown) ?? null;
  const label = shown ?? "Default";
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
      <PopoverContent align="start" side="top" className="w-80 space-y-3 p-3">
        <Field label="Theme">
          <ThemeList
            state={themes}
            value={style.theme}
            defaultTheme={defaultTheme}
            onChange={(theme) => set({ theme })}
          />
        </Field>
        <Field label="Format">
          <Segmented label="Format" options={FORMATS} value={style.format} onChange={(format) => set({ format })} />
        </Field>
        <Field label="Length">
          <LengthField value={style.lengthSecs} onChange={(lengthSecs) => set({ lengthSecs })} />
        </Field>
        <SetupLine />
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

/** The narration voice, and anything still missing before a video can be
 *  made (the voice, Kokoro's packs, the word-timing model), each with the
 *  Settings page that fixes it. */
function SetupLine() {
  const setup = useSetupProblems();
  const problems = setup === undefined ? [] : setup === null ? null : setup.problems;
  if (problems === null) {
    return <div className="border-t border-border-subtle px-0.5 pt-2.5 text-[12px] text-warning">The voice settings could not be read.</div>;
  }
  if (problems.length === 0) {
    return (
      <div className="flex items-center gap-2 border-t border-border-subtle px-0.5 pt-2.5 text-[12px]">
        <Microphone size={13} className="shrink-0 text-muted-foreground" />
        <span className="min-w-0 flex-1 truncate text-muted-foreground">
          Voice <span className="text-foreground">{setup?.voice.voice ?? ""}</span>
        </span>
        <SettingsLink href={NARRATION_SETTINGS_HREF} />
      </div>
    );
  }
  return (
    <div className="space-y-1.5 border-t border-border-subtle px-0.5 pt-2.5 text-[12px]">
      {problems.map((p) => (
        <div key={p.id} className="flex items-start gap-2">
          <Microphone size={13} className="mt-0.5 shrink-0 text-warning" />
          <span role="alert" className="min-w-0 flex-1 text-warning">
            {p.message}
          </span>
          <SettingsLink href={p.href} />
        </div>
      ))}
    </div>
  );
}

function SettingsLink({ href }: { href: string }) {
  const navigate = useNavigate();
  return (
    <button
      type="button"
      data-tab-href={href}
      onClick={() => navigate(href)}
      className="shrink-0 cursor-pointer text-brand underline-offset-2 hover:underline"
    >
      Settings
    </button>
  );
}
