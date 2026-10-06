import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { DotsSixVertical } from "@phosphor-icons/react";
import { GroqDialog } from "./GroqDialog";
import { LocalWhisperDialog, useWhisperDownloads, whisperStatus } from "./LocalWhisper";
import { OnDeviceSpeechDialog, speechStatus, type SpeechStatus } from "./OnDeviceSpeech";
import { Section } from "@/pages/settings/section";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { DRAG_SURFACE, useStripReorder } from "@/hooks/usePointerDrag";
import { cn } from "@/lib/utils";
import {
  appleLocale,
  appleSpeechStatus,
  DEFAULT_ENGINE_ORDER,
  hasGroqKey,
  languageOptions,
  languageValue,
  localeName,
  readTranscribeSettings,
  whisperModels,
  writeTranscribeSettings,
  type TranscribeEngine,
  type TranscribeSettings,
  type TranscribeSettingsPatch,
  type WhisperCatalogue,
} from "@/lib/transcribe";

/** The engine list's names; `ENGINE_LABELS` is the mid-sentence form. */
const NAMES: Record<TranscribeEngine, string> = {
  groq: "Groq",
  whisper: "Local Whisper",
  apple: "On-device speech",
};

/** The page's own copy of a patch, so a drop or a switch lands in one commit. */
function apply(settings: TranscribeSettings, patch: TranscribeSettingsPatch): TranscribeSettings {
  return {
    order: patch.order ?? settings.order,
    language: patch.language ?? settings.language,
    groq: { ...settings.groq, ...patch.groq },
    whisper: { ...settings.whisper, ...patch.whisper },
    apple: { ...settings.apple, ...patch.apple },
  };
}

function switched(engine: TranscribeEngine, enabled: boolean): TranscribeSettingsPatch {
  if (engine === "groq") return { groq: { enabled } };
  return engine === "whisper" ? { whisper: { enabled } } : { apple: { enabled } };
}

/** On-device speech lists no locales: the select falls back to bare codes. */
const NO_LOCALES: readonly string[] = [];

/**
 * Settings → Transcription: one language for every engine, and the engines in
 * the order a run tries them — each with its switch and a dialog for its own
 * setup (`GroqDialog`, `LocalWhisperDialog`, `OnDeviceSpeechDialog`). Rust
 * reads the `transcribe` row at the start of every run. Nothing here is
 * billed: saving a Groq key lists Groq's models, which is free, the speech
 * status only asks the OS what it has, and the Whisper listing reads the
 * models directory; only a model download the user starts goes online.
 */
export function TranscriptionSection() {
  const [settings, setSettings] = useState<TranscribeSettings | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [connected, setConnected] = useState<boolean | null>(null);
  const [speech, setSpeech] = useState<SpeechStatus>(undefined);
  const [catalogue, setCatalogue] = useState<WhisperCatalogue | null>(null);
  const [dialog, setDialog] = useState<TranscribeEngine | null>(null);
  // Writes are read-modify-write of one row: run them one at a time.
  const writes = useRef<Promise<unknown>>(Promise.resolve());

  const listModels = useCallback(() => {
    whisperModels()
      .then(setCatalogue)
      .catch((cause) => {
        console.error("Whisper model listing failed", cause);
        setNote(String(cause));
      });
  }, []);
  const downloads = useWhisperDownloads(catalogue, listModels);

  useEffect(() => {
    let cancelled = false;
    hasGroqKey()
      .then((present) => {
        if (!cancelled) setConnected(present);
      })
      .catch((cause) => console.error("Groq key check failed", cause));
    appleSpeechStatus()
      .then((status) => {
        if (!cancelled) setSpeech(status);
      })
      .catch((cause) => {
        console.error("On-device speech status failed", cause);
        if (!cancelled) setSpeech(null);
      });
    readTranscribeSettings()
      .then((read) => {
        if (!cancelled) setSettings(read);
      })
      .catch((cause) => {
        console.error("Transcription settings read failed", cause);
        if (!cancelled) setNote(String(cause));
      });
    listModels();
    return () => {
      cancelled = true;
    };
  }, [listModels]);

  const save = (patch: TranscribeSettingsPatch) => {
    setNote(null);
    setSettings((current) => (current ? apply(current, patch) : current));
    writes.current = writes.current
      .then(() => writeTranscribeSettings(patch))
      .catch((cause) => {
        console.error("Transcription setting failed", cause);
        setNote(String(cause));
        // Show what is stored, not what failed to be.
        return readTranscribeSettings().then(setSettings, () => {});
      });
  };

  const order = settings?.order ?? [...DEFAULT_ENGINE_ORDER];
  const language = settings?.language ?? null;
  const defaultLocale = speech?.available ? speech.defaultLocale : null;

  const status: Record<TranscribeEngine, { text: string; ready: boolean }> = {
    groq:
      connected === null
        ? { text: "", ready: false }
        : { text: connected ? "Connected" : "No key", ready: connected },
    whisper: whisperStatus(catalogue, settings?.whisper ?? null, downloads.downloads),
    apple: speechStatus(speech, language),
  };
  const settled = settings !== null && connected !== null && speech !== undefined && catalogue !== null;
  const anyReady = order.some((e) => settings?.[e].enabled && status[e].ready);

  return (
    <Section
      title="Transcription"
      description="Videos without captions are transcribed on Groq's free tier or on this Mac."
    >
      <LanguageRow
        language={language}
        speech={speech}
        defaultLocale={defaultLocale}
        disabled={!settings}
        onChange={(next) => save({ language: next })}
      />

      <div className="mt-3">
        <p className="text-xs text-foreground">Engines</p>
        <p id="transcribe-engines-hint" className="text-[11px] text-muted-foreground">
          Tried top to bottom: the next one answers only when the one above isn't set up or hits
          its limit.
        </p>
        <EngineList
          order={order}
          settings={settings}
          status={status}
          onReorder={(next) => save({ order: next })}
          onToggle={(engine, enabled) => save(switched(engine, enabled))}
          onOpen={setDialog}
        />
        {settled && !anyReady ? (
          <p className="mt-2 text-[11px] text-warning">
            No engine is set up, so videos without captions stay without a transcript.
          </p>
        ) : null}
        {note ? (
          <p className="mt-2 text-[11px] leading-relaxed text-destructive" data-selectable>
            {note}
          </p>
        ) : null}
      </div>

      <GroqDialog
        open={dialog === "groq"}
        onOpenChange={(open) => setDialog(open ? "groq" : null)}
        connected={connected}
        onConnected={setConnected}
      />
      <LocalWhisperDialog
        open={dialog === "whisper"}
        onOpenChange={(open) => setDialog(open ? "whisper" : null)}
        catalogue={catalogue}
        settings={settings?.whisper ?? null}
        downloads={downloads}
        onChange={(patch) => save({ whisper: patch })}
        onModelsChanged={listModels}
      />
      <OnDeviceSpeechDialog
        open={dialog === "apple"}
        onOpenChange={(open) => setDialog(open ? "apple" : null)}
        status={speech}
        language={language}
      />
    </Section>
  );
}

/** One select for every engine. The default (`null`) shows as on-device
 *  speech's default locale, and is stored only once the user picks. */
function LanguageRow({ language, speech, defaultLocale, disabled, onChange }: {
  language: string | null;
  speech: SpeechStatus;
  defaultLocale: string | null;
  disabled: boolean;
  onChange: (language: string) => void;
}) {
  const value = languageValue(language, defaultLocale);
  const supported = speech?.available ? speech.supported : NO_LOCALES;
  const options = useMemo(() => languageOptions(supported, value), [supported, value]);
  const shown = options.find((o) => o.value === value)?.name ?? localeName(value);
  const fallback = appleLocale("auto", defaultLocale);

  return (
    <div className="flex items-center justify-between gap-4 py-2">
      <div className="min-w-0">
        <p className="text-xs text-foreground">Language</p>
        <p className="text-[11px] text-muted-foreground">
          {value === "auto"
            ? `Whisper and Groq judge it from the first 30 seconds heard.${
                fallback ? ` On-device speech has no auto-detect and uses ${localeName(fallback)}.` : ""
              }`
            : "Every engine transcribes in this language."}
        </p>
      </div>
      <Select value={value} disabled={disabled} onValueChange={onChange}>
        <SelectTrigger aria-label="Transcription language" size="sm" className="h-7 w-56 text-xs">
          <SelectValue placeholder="—">{shown}</SelectValue>
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={option.value} value={option.value} className="text-xs">
              {option.name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

/**
 * The engines as one bordered group, in run order. A row drags to a new
 * place (`useStripReorder`, the sidebar rail's gesture); its handle also
 * moves it with the arrow keys.
 */
function EngineList({ order, settings, status, onReorder, onToggle, onOpen }: {
  order: TranscribeEngine[];
  settings: TranscribeSettings | null;
  status: Record<TranscribeEngine, { text: string; ready: boolean }>;
  onReorder: (order: TranscribeEngine[]) => void;
  onToggle: (engine: TranscribeEngine, enabled: boolean) => void;
  onOpen: (engine: TranscribeEngine) => void;
}) {
  const [announcement, setAnnouncement] = useState("");
  const handles = useRef(new Map<TranscribeEngine, HTMLButtonElement>());
  // React may move the focused row's node to reorder, which blurs it.
  const refocus = useRef<TranscribeEngine | null>(null);
  useEffect(() => {
    if (!refocus.current) return;
    handles.current.get(refocus.current)?.focus();
    refocus.current = null;
  }, [order]);
  const strip = useStripReorder({
    keys: order,
    axis: "y",
    // Equal-height rows: a clamped centre never passes the end ones.
    swapOn: "edge",
    onDrop: ({ order }) => onReorder(order),
  });

  const move = (engine: TranscribeEngine, by: -1 | 1) => {
    const from = order.indexOf(engine);
    const to = from + by;
    if (to < 0 || to >= order.length) return;
    const next = [...order];
    next.splice(to, 0, next.splice(from, 1)[0]);
    refocus.current = engine;
    onReorder(next);
    setAnnouncement(`${NAMES[engine]} moved to position ${to + 1} of ${order.length}`);
  };

  const onKey = (e: KeyboardEvent<HTMLButtonElement>, engine: TranscribeEngine) => {
    if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
    e.preventDefault();
    move(engine, e.key === "ArrowUp" ? -1 : 1);
  };

  return (
    <>
      <ol
        aria-label="Transcription engines, in the order they are tried"
        className={cn(
          "mt-2 divide-y divide-border-subtle rounded-lg border border-border-subtle",
          DRAG_SURFACE,
        )}
      >
        {order.map((engine, i) => {
          const grabbed = strip.drag?.key === engine;
          const enabled = settings?.[engine].enabled ?? false;
          const { text, ready } = status[engine];
          const verb = engine === "apple" ? (ready ? "Manage" : "Details") : ready ? "Manage" : "Set up";
          return (
            <li
              key={engine}
              ref={strip.itemRef(engine)}
              onPointerDown={(e) => strip.onPointerDown(e, engine)}
              // Swallow the click a drag's release raises, before a button hears it.
              onClickCapture={(e) => {
                if (strip.didDrag()) e.stopPropagation();
              }}
              style={strip.styleFor(engine, i)}
              className={cn(
                "flex items-center gap-2 bg-card py-2 pr-2.5 pl-1.5 first:rounded-t-lg last:rounded-b-lg",
                grabbed
                  ? "relative z-10 rounded-lg shadow-md"
                  : strip.drag && "transition-transform duration-200 ease-out",
              )}
            >
              <button
                ref={(node) => {
                  if (node) handles.current.set(engine, node);
                  else handles.current.delete(engine);
                }}
                type="button"
                aria-label={`${NAMES[engine]}, position ${i + 1} of ${order.length}: the arrow keys move it`}
                aria-describedby="transcribe-engines-hint"
                onKeyDown={(e) => onKey(e, engine)}
                className={cn(
                  "flex size-6 shrink-0 items-center justify-center rounded-md text-muted-foreground/70",
                  "cursor-grab outline-none hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50",
                  grabbed && "cursor-grabbing",
                )}
              >
                <DotsSixVertical size={14} weight="bold" />
              </button>
              <span className="w-3 shrink-0 text-[11px] text-muted-foreground tabular-nums">{i + 1}</span>
              <div className={cn("min-w-0 flex-1", !enabled && settings && "opacity-60")}>
                <p className="text-xs text-foreground">{NAMES[engine]}</p>
                <p className="truncate text-[11px] text-muted-foreground">{text || "\u00a0"}</p>
              </div>
              <Switch
                aria-label={`Use ${NAMES[engine]}`}
                checked={enabled}
                disabled={!settings}
                onCheckedChange={(checked) => onToggle(engine, checked)}
                className="shrink-0"
              />
              <Button variant="outline" size="xs" className="w-16 shrink-0" onClick={() => onOpen(engine)}>
                {verb}
              </Button>
            </li>
          );
        })}
      </ol>
      <p className="sr-only" aria-live="polite">
        {announcement}
      </p>
    </>
  );
}
