import { Play, Stop } from "@phosphor-icons/react";
import { percent, type Download } from "@/components/settings/transcription/LocalWhisper";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import type { KokoroVoice } from "@/lib/studio/kokoroVoices";
import type { SamplePhase } from "../useClipPlayer";
import { VOICE_COLS } from "./constants";

/** Binary, since a voice file is 510 × 1024 bytes. */
export const voiceSize = (bytes: number) => `${Math.round(bytes / 1024)} KB`;

/**
 * One voice: its name (marked when narration uses it), accent and gender,
 * grade, size and status, then Sample and Download (a percentage and Cancel
 * while it downloads, Delete once it is on disk). The row's own error sits
 * beneath it.
 */
export function VoiceRow({ voice, chosen, download, error, sample, canSample, onSample, onDownload, onCancel, onDelete }: {
  voice: KokoroVoice;
  /** Narration uses this voice. */
  chosen: boolean;
  download: Download | undefined;
  error: string | undefined;
  sample: SamplePhase;
  /** The model and pronunciation pack are downloaded. */
  canSample: boolean;
  onSample: () => void;
  onDownload: () => void;
  onCancel: () => void;
  onDelete: () => void;
}) {
  const present = voice.status === "present";
  const done = download ? percent(download) : 0;

  return (
    <div className="px-3 py-1.5">
      <div className={`${VOICE_COLS} items-center`}>
        <span className="flex min-w-0 items-baseline gap-1.5 text-xs text-foreground">
          <span className="truncate">{voice.name}</span>
          {chosen ? <span className="shrink-0 text-[11px] text-brand">In use</span> : null}
        </span>
        <span className="truncate text-[11px] text-muted-foreground">
          {voice.accent}, {voice.gender}
        </span>
        <span className="text-[11px] text-muted-foreground tabular-nums">{voice.grade}</span>
        <span className="text-[11px] text-muted-foreground tabular-nums">{voiceSize(voice.bytes)}</span>
        <span className="text-[11px] tabular-nums">
          {download ? (
            <span className="text-brand">{done}%</span>
          ) : present ? (
            <span className="text-foreground">Downloaded</span>
          ) : (
            <span className="text-muted-foreground">Not downloaded</span>
          )}
        </span>
        <span className="flex items-center justify-end gap-1.5">
          {sample === "playing" ? (
            <Button variant="outline" size="xs" className="w-[5.5rem]" onClick={onSample}>
              <Stop size={12} weight="fill" />
              Stop
            </Button>
          ) : (
            <Button
              variant="outline"
              size="xs"
              className="w-[5.5rem]"
              disabled={!present || !canSample || sample === "making"}
              onClick={onSample}
            >
              <Play size={12} weight="fill" />
              {sample === "making" ? "Making…" : "Sample"}
            </Button>
          )}
          {download ? (
            <Button variant="outline" size="xs" className="w-[4.5rem]" onClick={onCancel}>
              Cancel
            </Button>
          ) : present ? (
            <Button
              variant="ghost"
              size="xs"
              className="w-[4.5rem] text-muted-foreground"
              onClick={onDelete}
            >
              Delete
            </Button>
          ) : (
            <Button variant="outline" size="xs" className="w-[4.5rem]" onClick={onDownload}>
              Download
            </Button>
          )}
        </span>
      </div>
      {download ? <Progress value={done} className="mt-1.5 h-1" indicatorClassName="bg-brand" /> : null}
      {error ? (
        <p className="mt-1.5 text-[11px] leading-relaxed text-destructive" data-selectable>
          {error}
        </p>
      ) : null}
    </div>
  );
}
