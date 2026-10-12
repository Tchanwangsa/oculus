import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent, ReactNode } from "react";
import { Section } from "@/components/settings/shared/section";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import {
  listVoiceModels,
  readVoiceSettings,
  SPEED_MAX,
  SPEED_MIN,
  SPEED_STEP,
  writeVoiceSettings,
  type VoiceSettings,
} from "@/lib/studio/voice";

type Field = "baseUrl" | "model" | "voice" | "speed";

/** `1×`, `1.25×`: trailing zeros trimmed. */
const fmtSpeed = (s: number) => `${Number(s.toFixed(2))}×`;

/** Enter commits a text field by blurring it, so blur is the one save path. */
const blurOnEnter = (e: KeyboardEvent<HTMLInputElement>) => {
  if (e.key === "Enter") e.currentTarget.blur();
};

/**
 * Settings → Voice: the TTS server Studio narrates with, written to the
 * `voice` row by Rust, which validates it and answers with what it stored.
 * The one network call is "List models" (`GET /v1/models`, free); nothing
 * here synthesises, so there is no sample button. Keyless servers only.
 */
export function NarrationVoiceSection() {
  const [settings, setSettings] = useState<VoiceSettings | null>(null);
  const [urlDraft, setUrlDraft] = useState("");
  const [modelDraft, setModelDraft] = useState("");
  const [voiceDraft, setVoiceDraft] = useState("");
  const [speed, setSpeed] = useState(1);
  const [errors, setErrors] = useState<Partial<Record<Field, string>>>({});
  const [note, setNote] = useState<string | null>(null);
  const [models, setModels] = useState<string[] | null>(null);
  const [listing, setListing] = useState(false);
  const [listError, setListError] = useState<string | null>(null);
  // The row as last stored; each write sends all of it, one at a time.
  const saved = useRef<VoiceSettings | null>(null);
  const writes = useRef<Promise<unknown>>(Promise.resolve());

  const show = (stored: VoiceSettings) => {
    saved.current = stored;
    setSettings(stored);
    setUrlDraft(stored.baseUrl ?? "");
    setModelDraft(stored.model ?? "");
    setVoiceDraft(stored.voice ?? "");
    setSpeed(stored.speed);
  };

  useEffect(() => {
    let cancelled = false;
    readVoiceSettings()
      .then((read) => {
        if (!cancelled) show(read);
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

  const save = (field: Field, patch: Partial<VoiceSettings>) => {
    writes.current = writes.current.then(async () => {
      const current = saved.current;
      if (!current) return;
      try {
        const stored = await writeVoiceSettings({ ...current, ...patch });
        saved.current = stored;
        setSettings(stored);
        setError(field, null);
        // Show the stored form (trimmed, normalised) of the field just saved.
        if (field === "baseUrl") setUrlDraft(stored.baseUrl ?? "");
        if (field === "model") setModelDraft(stored.model ?? "");
        if (field === "voice") setVoiceDraft(stored.voice ?? "");
        if (field === "speed") setSpeed(stored.speed);
      } catch (cause) {
        console.error("Voice setting failed", cause);
        setError(field, String(cause));
      }
    });
  };

  /** Save a text field when its draft differs from what is stored. */
  const commit = (field: "baseUrl" | "model" | "voice", draft: string) => {
    const value = draft.trim() || null;
    if (value === (saved.current?.[field] ?? null)) {
      setError(field, null);
      return;
    }
    if (field === "baseUrl") {
      // Another server's list says nothing about this one.
      setModels(null);
      setListError(null);
    }
    save(field, { [field]: value });
  };

  // The field as typed: Rust normalises it, and a bad one fails here too.
  const list = async () => {
    const base = urlDraft.trim();
    if (!base) return;
    setListing(true);
    setListError(null);
    try {
      setModels(await listVoiceModels(base));
    } catch (cause) {
      setModels(null);
      setListError(String(cause));
    } finally {
      setListing(false);
    }
  };

  const loaded = settings !== null;
  // The stored model stays choosable even when the server does not list it.
  const options =
    models && models.length > 0
      ? settings?.model && !models.includes(settings.model)
        ? [settings.model, ...models]
        : models
      : null;

  return (
    <Section
      title="Narration voice"
      description="Studio reads narration with an OpenAI-compatible text-to-speech server you run, e.g. Kokoro-FastAPI or mlx-audio."
    >
      <div className="space-y-1">
        <Row
          label="Server URL"
          hint="Where the server answers, with or without /v1."
          error={errors.baseUrl}
        >
          <Input
            aria-label="TTS server URL"
            type="url"
            autoComplete="off"
            spellCheck={false}
            value={urlDraft}
            disabled={!loaded}
            onChange={(e) => setUrlDraft(e.target.value)}
            onBlur={() => commit("baseUrl", urlDraft)}
            onKeyDown={blurOnEnter}
            placeholder="http://127.0.0.1:8880"
            className="h-7 w-80 text-xs"
          />
        </Row>

        <Row
          label="Model"
          hint={
            options
              ? "The models the server lists."
              : "The server's model id, e.g. kokoro. List models asks the server for its ids."
          }
          error={errors.model ?? listError}
          note={models?.length === 0 ? "The server listed no models: type the model id." : null}
        >
          <div className="flex w-80 items-center gap-2">
            {options ? (
              <Select
                value={settings?.model ?? ""}
                disabled={!loaded}
                onValueChange={(model) => {
                  setModelDraft(model);
                  save("model", { model });
                }}
              >
                <SelectTrigger aria-label="TTS model" size="sm" className="h-7 min-w-0 flex-1 text-xs">
                  <SelectValue placeholder="Choose a model" />
                </SelectTrigger>
                <SelectContent>
                  {options.map((id) => (
                    <SelectItem key={id} value={id} className="text-xs">
                      {id}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            ) : (
              <Input
                aria-label="TTS model"
                autoComplete="off"
                spellCheck={false}
                value={modelDraft}
                disabled={!loaded}
                onChange={(e) => setModelDraft(e.target.value)}
                onBlur={() => commit("model", modelDraft)}
                onKeyDown={blurOnEnter}
                placeholder="kokoro"
                className="h-7 min-w-0 flex-1 text-xs"
              />
            )}
            <Button
              variant="outline"
              size="xs"
              className="w-[5.5rem] shrink-0"
              disabled={!loaded || !urlDraft.trim() || listing}
              onClick={() => void list()}
            >
              {listing ? "Listing…" : "List models"}
            </Button>
          </div>
        </Row>

        <Row
          label="Voice"
          hint="The server's voice name, e.g. af_heart. Servers can't list their voices, so type it."
          error={errors.voice}
        >
          <Input
            aria-label="TTS voice"
            autoComplete="off"
            spellCheck={false}
            value={voiceDraft}
            disabled={!loaded}
            onChange={(e) => setVoiceDraft(e.target.value)}
            onBlur={() => commit("voice", voiceDraft)}
            onKeyDown={blurOnEnter}
            placeholder="af_heart"
            className="h-7 w-80 text-xs"
          />
        </Row>

        <Row label="Speed" hint="1× is the voice's own pace." error={errors.speed}>
          <div className="flex w-80 items-center gap-3">
            <Slider
              aria-label="Narration speed"
              min={SPEED_MIN}
              max={SPEED_MAX}
              step={SPEED_STEP}
              value={[speed]}
              disabled={!loaded}
              onValueChange={([value]) => setSpeed(value)}
              onValueCommit={([value]) => save("speed", { speed: value })}
            />
            <span className="w-10 shrink-0 text-right text-xs text-foreground tabular-nums">
              {fmtSpeed(speed)}
            </span>
          </div>
        </Row>

        <p className="pt-1 text-[11px] leading-relaxed text-muted-foreground">
          API keys aren't supported yet: use a server that needs none.
        </p>

        {note ? (
          <p className="pt-1 text-[11px] leading-relaxed text-destructive" data-selectable>
            {note}
          </p>
        ) : null}
      </div>
    </Section>
  );
}

/** A label and hint on the left, the control on the right, and the row's
 *  own error or note beneath. */
function Row({ label, hint, error, note, children }: {
  label: string;
  hint: string;
  error?: string | null;
  note?: string | null;
  children: ReactNode;
}) {
  return (
    <div className="py-2">
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0">
          <p className="text-xs text-foreground">{label}</p>
          <p className="text-[11px] text-muted-foreground">{hint}</p>
        </div>
        <div className="shrink-0">{children}</div>
      </div>
      {error ? (
        <p className="mt-2 text-[11px] leading-relaxed text-destructive" data-selectable>
          {error}
        </p>
      ) : note ? (
        <p className="mt-2 text-[11px] leading-relaxed text-muted-foreground">{note}</p>
      ) : null}
    </div>
  );
}
