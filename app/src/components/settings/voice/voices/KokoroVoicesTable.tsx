import { useRef, useState } from "react";
import { HeaderLabels } from "@/components/ui/table/GridTable";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import type { KokoroVoice } from "@/lib/studio/kokoroVoices";
import { cn } from "@/lib/utils";
import { useVoiceSample } from "../useVoiceSample";
import { VOICE_COLS } from "./constants";
import type { KokoroVoices } from "./useKokoroVoices";
import { VoiceRow, voiceSize } from "./VoiceRow";
import { useScrollFade } from "@/hooks/ui/useScrollFade";

/**
 * Kokoro's voices as a table in the grammar of `SyncHistoryTable`: a row each
 * with its grade, size and status, Download or Delete, and Sample, which
 * speaks a fixed sentence at the chosen speed and needs the model and
 * pronunciation pack. The header counts what is downloaded and downloads
 * the rest. Deleting the voice narration uses asks first.
 */
export function KokoroVoicesTable({ files, chosen, speed, canSample }: {
  files: KokoroVoices;
  /** The `voice` setting. */
  chosen: string | null;
  speed: number;
  /** The model and pronunciation pack are downloaded. */
  canSample: boolean;
}) {
  const { voices, downloads, errors, loadError, queue } = files;
  const sample = useVoiceSample();
  const [deleting, setDeleting] = useState<KokoroVoice | null>(null);
  const [busy, setBusy] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  useScrollFade(scrollRef, "x");

  const present = voices?.filter((v) => v.status === "present").length ?? 0;
  const missing = voices?.filter((v) => v.status === "missing").length ?? 0;

  const onDelete = async (voice: KokoroVoice) => {
    setBusy(true);
    await files.remove(voice.id);
    setBusy(false);
    setDeleting(null);
  };

  const askDelete = (voice: KokoroVoice) => {
    if (voice.id === chosen) setDeleting(voice);
    else void files.remove(voice.id);
  };

  return (
    <div className="py-2">
      <div className="settings-row flex items-center justify-between gap-4">
        <div className="min-w-0">
          <p className="text-xs text-foreground">Voices</p>
          <p className="text-[11px] text-muted-foreground tabular-nums">
            {[
              voices && `${present} of ${voices.length} downloaded · ${voiceSize(voices[0]?.bytes ?? 0)} each`,
              "Samples need both packs.",
            ]
              .filter(Boolean)
              .join(" · ")}
          </p>
        </div>
        {queue ? (
          <div className="settings-actions flex shrink-0 items-center gap-2">
            <span className="text-[11px] text-brand tabular-nums">
              {queue.done + 1} of {queue.total}
            </span>
            <Button variant="outline" size="xs" onClick={files.cancelAll}>
              Cancel
            </Button>
          </div>
        ) : (
          <Button
            variant="outline"
            size="xs"
            className="shrink-0"
            disabled={!voices || missing === 0}
            onClick={() => void files.downloadAll()}
          >
            Download all
          </Button>
        )}
      </div>

      <div ref={scrollRef} className="mt-2 overflow-x-auto rounded-lg border border-border-subtle bg-card" tabIndex={0} role="region" aria-label="Voices; scroll horizontally for more columns">
        <div className="min-w-[680px]">
          <div className={cn(VOICE_COLS, "border-b border-border-subtle px-3 py-2")}>
            <HeaderLabels labels={["Voice", "Accent", "Grade", "Size", "Status", ""]} />
          </div>
          <div className="divide-y divide-border-subtle">
            {voices?.map((voice) => (
              <VoiceRow
                key={voice.id}
                voice={voice}
                chosen={voice.id === chosen}
                download={downloads[voice.id]}
                error={errors[voice.id] ?? (sample.error?.key === voice.id ? sample.error.message : undefined)}
                sample={sample.phaseOf(voice.id)}
                canSample={canSample}
                onSample={() =>
                  sample.phaseOf(voice.id) === "playing" ? sample.stop() : void sample.play(voice.id, speed)
                }
                onDownload={() => files.start(voice)}
                onCancel={() => files.cancel(voice.id)}
                onDelete={() => askDelete(voice)}
              />
            )) ?? <p className="px-3 py-6 text-center text-xs text-muted-foreground">Reading the voices…</p>}
          </div>
        </div>
      </div>
      {loadError ? (
        <p className="mt-2 text-[11px] leading-relaxed text-destructive" data-selectable>
          {loadError}
        </p>
      ) : null}

      <ConfirmDialog
        open={deleting !== null}
        onCancel={() => setDeleting(null)}
        onConfirm={() => deleting && void onDelete(deleting)}
        title={`Delete ${deleting?.name ?? "voice"}?`}
        description="Narration uses this voice. It can't narrate until you download it again here or choose another."
        confirmLabel="Delete"
        busyLabel="Deleting…"
        busy={busy}
      />
    </div>
  );
}
