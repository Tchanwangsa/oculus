import { Radio } from "@/components/settings/transcription/LocalWhisper";
import { cn } from "@/lib/utils";
import type { VoiceEngine } from "@/lib/studio/voice";

export const ENGINE_NAMES: Record<VoiceEngine, string> = {
  kokoro: "Kokoro (built in)",
  server: "TTS server",
};

const ENGINES: VoiceEngine[] = ["kokoro", "server"];

/**
 * The narration engines as one bordered group, as Settings → Transcription
 * lists its engines, but a choice of one: a video has one engine and no
 * fallback. Nothing is chosen until the user picks, since Kokoro needs a
 * download and a server a URL. A whole row picks its engine.
 */
export function EngineChoice({ engine, status, disabled, onChoose }: {
  engine: VoiceEngine | null;
  /** Each engine's one line: what it would use, or what is missing. */
  status: Record<VoiceEngine, string>;
  disabled: boolean;
  onChoose: (engine: VoiceEngine) => void;
}) {
  const choose = (next: VoiceEngine) => {
    if (!disabled && next !== engine) onChoose(next);
  };

  return (
    <div
      role="radiogroup"
      aria-label="Narration engine"
      className="mt-2 divide-y divide-border-subtle rounded-lg border border-border-subtle"
    >
      {ENGINES.map((id) => (
        <div
          key={id}
          onClick={() => choose(id)}
          className={cn(
            "flex items-center gap-2.5 bg-card px-2.5 py-2 first:rounded-t-lg last:rounded-b-lg",
            !disabled && "cursor-pointer",
          )}
        >
          {/* Its click (a press, or Space on the focused dot) bubbles to the row. */}
          <Radio label={ENGINE_NAMES[id]} checked={engine === id} onSelect={() => {}} />
          <div className="min-w-0 flex-1">
            <p className="text-xs text-foreground">{ENGINE_NAMES[id]}</p>
            <p className="text-[11px] text-muted-foreground tabular-nums">
              {status[id] || " "}
            </p>
          </div>
        </div>
      ))}
    </div>
  );
}
