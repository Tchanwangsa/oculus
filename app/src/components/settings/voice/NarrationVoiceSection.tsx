import { useEffect, useRef, useState } from "react";
import { Section } from "@/components/settings/shared/section";
import { modelSize, percent } from "@/components/settings/transcription/LocalWhisper";
import { Slider } from "@/components/ui/slider";
import {
  readVoiceSettings,
  SPEED_MAX,
  SPEED_MIN,
  SPEED_STEP,
  writeVoiceSettings,
  type VoiceEngine,
  type VoiceSettings,
} from "@/lib/studio/voice";
import {
  cancelVoicePacksDownload,
  deleteVoicePacks,
  downloadVoicePacks,
  onVoicePacksProgress,
  voicePacksStatus,
  type VoicePacksStatus,
} from "@/lib/studio/voicePacks";
import { EngineChoice } from "./EngineChoice";
import { KokoroFields } from "./KokoroFields";
import { ServerFields } from "./ServerFields";
import { SettingRow } from "./SettingRow";
import { usePackDownload, type PackApi } from "./usePackDownload";
import { VoiceSampleRow } from "./VoiceSampleRow";

type Field = "engine" | "baseUrl" | "model" | "voice" | "speed";

/** Each field's last refusal, shown under its row. */
export type VoiceErrors = Partial<Record<Field, string>>;

/** Write `patch` over the stored row; resolves to what was stored, or null
 *  when it was refused (the reason lands in `VoiceErrors[field]`). */
export type SaveVoice = (field: Field, patch: Partial<VoiceSettings>) => Promise<VoiceSettings | null>;

const VOICE_PACKS: PackApi<VoicePacksStatus> = {
  name: "Kokoro packs",
  status: voicePacksStatus,
  download: downloadVoicePacks,
  cancel: cancelVoicePacksDownload,
  remove: deleteVoicePacks,
  onProgress: onVoicePacksProgress,
};

/** `1×`, `1.25×`: trailing zeros trimmed. */
const fmtSpeed = (s: number) => `${Number(s.toFixed(2))}×`;

/**
 * Settings → Voice: the engine Studio narrates with (Kokoro in the app, or a
 * keyless OpenAI-compatible TTS server), its voice and the speed, written to
 * the `voice` row by Rust, which validates it and answers with what it
 * stored. Nothing here is billed: listing a server's models is free, and
 * only Kokoro, on this Mac, plays a sample.
 */
export function NarrationVoiceSection() {
  const [settings, setSettings] = useState<VoiceSettings | null>(null);
  const [speed, setSpeed] = useState(1);
  const [errors, setErrors] = useState<VoiceErrors>({});
  const [note, setNote] = useState<string | null>(null);
  const packs = usePackDownload(VOICE_PACKS);
  // The row as last stored; each write sends all of it, one at a time.
  const saved = useRef<VoiceSettings | null>(null);
  const writes = useRef<Promise<unknown>>(Promise.resolve());

  useEffect(() => {
    let cancelled = false;
    readVoiceSettings()
      .then((read) => {
        if (cancelled) return;
        saved.current = read;
        setSettings(read);
        setSpeed(read.speed);
      })
      .catch((cause) => {
        console.error("Voice settings read failed", cause);
        if (!cancelled) setNote(String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const setError = (field: Field, error: string | null) =>
    setErrors(({ [field]: _, ...rest }) => (error ? { ...rest, [field]: error } : rest));

  const save: SaveVoice = (field, patch) => {
    const write = writes.current.then(async () => {
      const current = saved.current;
      if (!current) return null;
      try {
        const stored = await writeVoiceSettings({ ...current, ...patch });
        saved.current = stored;
        setSettings(stored);
        setError(field, null);
        if (field === "speed") setSpeed(stored.speed);
        return stored;
      } catch (cause) {
        console.error("Voice setting failed", cause);
        setError(field, String(cause));
        return null;
      }
    });
    writes.current = write;
    return write;
  };

  const engine = settings?.engine ?? null;
  const ready = packs.status?.status === "present";
  const status: Record<VoiceEngine, string> = {
    kokoro: !packs.status
      ? ""
      : packs.download
        ? `Downloading the packs · ${percent(packs.download)}%`
        : ready
          ? "Runs on this Mac, offline"
          : `Runs on this Mac, offline, after a ${modelSize(packs.status.bytes)} download`,
    server: !settings
      ? ""
      : settings.baseUrl
        ? [settings.baseUrl, settings.model].filter(Boolean).join(" · ")
        : "No server set",
  };

  return (
    <Section
      title="Narration voice"
      description="Studio reads narration with Kokoro on this Mac, or with a text-to-speech server you run."
    >
      <p className="text-xs text-foreground">Engine</p>
      <p className="text-[11px] text-muted-foreground">
        Every video uses this one unless its own voice says otherwise.
      </p>
      <EngineChoice
        engine={engine}
        status={status}
        disabled={!settings}
        onChoose={(next) => void save("engine", { engine: next })}
      />
      {errors.engine ? (
        <p className="mt-2 text-[11px] leading-relaxed text-destructive" data-selectable>
          {errors.engine}
        </p>
      ) : settings && !engine ? (
        <p className="mt-2 text-[11px] text-warning">
          No engine is chosen, so Studio can't narrate a video.
        </p>
      ) : null}

      {settings && engine ? (
        <div className="mt-3 space-y-1">
          {engine === "kokoro" ? (
            <KokoroFields settings={settings} errors={errors} save={save} packs={packs} />
          ) : (
            <ServerFields
              settings={settings}
              errors={errors}
              save={save}
              clearError={(field) => setError(field, null)}
            />
          )}

          <SettingRow label="Speed" hint="1× is the voice's own pace." error={errors.speed}>
            <div className="flex w-80 items-center gap-3">
              <Slider
                aria-label="Narration speed"
                min={SPEED_MIN}
                max={SPEED_MAX}
                step={SPEED_STEP}
                value={[speed]}
                onValueChange={([value]) => setSpeed(value)}
                onValueCommit={([value]) => void save("speed", { speed: value })}
              />
              <span className="w-10 shrink-0 text-right text-xs text-foreground tabular-nums">
                {fmtSpeed(speed)}
              </span>
            </div>
          </SettingRow>

          {engine === "kokoro" ? (
            <>
              <VoiceSampleRow voice={settings.voice} speed={speed} ready={ready} />
              <p className="pt-1 text-[11px] leading-relaxed text-muted-foreground">
                hexgrad/Kokoro-82M (onnx-community's int8 export), misaki's dictionary and
                PeterReid/graphemes_to_phonemes_en_us, Apache-2.0
              </p>
            </>
          ) : (
            <p className="pt-1 text-[11px] leading-relaxed text-muted-foreground">
              API keys aren't supported yet: use a server that needs none. A server has no sample
              here, since it could bill.
            </p>
          )}
        </div>
      ) : null}

      {note ? (
        <p className="pt-1 text-[11px] leading-relaxed text-destructive" data-selectable>
          {note}
        </p>
      ) : null}
    </Section>
  );
}
