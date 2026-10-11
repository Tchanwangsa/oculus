import { useState } from "react";
import { Section } from "@/components/settings/shared/section";
import { modelSize, percent } from "@/components/settings/transcription/LocalWhisper";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { Progress } from "@/components/ui/progress";
import { useAlignerDownload } from "./useAlignerDownload";

const MODEL_NAME = "wav2vec2 base (English)";

/**
 * The narration aligner's model, downloaded on request like a Local Whisper
 * model: Download, its progress and Cancel, then Delete. Narration cannot
 * time its words without it. The status is local; only Download goes online.
 */
export function WordTimingSection() {
  const { status, download, error, start, cancel, remove } = useAlignerDownload();
  const [confirm, setConfirm] = useState(false);
  const [deleting, setDeleting] = useState(false);

  const bytes = status?.bytes ?? 0;
  const done = download ? percent(download) : 0;
  const present = status?.status === "present";
  const line = !status
    ? " "
    : `${modelSize(bytes)} download · ${
        download ? "Downloading" : present ? "Downloaded" : "Not downloaded"
      }`;

  const onDelete = async () => {
    setDeleting(true);
    await remove();
    setDeleting(false);
    setConfirm(false);
  };

  return (
    <Section
      title="Word timing"
      description="Studio times each narrated word on this Mac, so a scene can bring things in on a word."
    >
      <div className="py-2">
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0">
            <p className="text-xs text-foreground">{MODEL_NAME}</p>
            <p className="text-[11px] text-muted-foreground tabular-nums">{line}</p>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            {download ? (
              <>
                <span className="w-9 text-right text-[11px] text-brand tabular-nums">{done}%</span>
                <Button variant="outline" size="xs" onClick={cancel}>
                  Cancel
                </Button>
              </>
            ) : present ? (
              <Button
                variant="ghost"
                size="xs"
                className="text-muted-foreground"
                onClick={() => setConfirm(true)}
              >
                Delete
              </Button>
            ) : (
              <Button variant="outline" size="xs" disabled={!status} onClick={start}>
                Download
              </Button>
            )}
          </div>
        </div>
        {download ? (
          <Progress value={done} className="mt-1.5 h-1" indicatorClassName="bg-brand" />
        ) : null}
        {error ? (
          <p className="mt-2 text-[11px] leading-relaxed text-destructive" data-selectable>
            {error}
          </p>
        ) : null}
        {status && !present && !download && !error ? (
          <p className="mt-2 text-[11px] leading-relaxed text-warning">
            Narration can't time its words until this is downloaded.
          </p>
        ) : null}
      </div>

      <p className="pt-1 text-[11px] leading-relaxed text-muted-foreground">
        facebook/wav2vec2-base-960h, Apache-2.0
      </p>

      <ConfirmDialog
        open={confirm}
        onCancel={() => setConfirm(false)}
        onConfirm={() => void onDelete()}
        title={`Delete ${MODEL_NAME}?`}
        description={`Frees ${modelSize(bytes)}. Narration can't time its words until it is downloaded again from here.`}
        confirmLabel="Delete"
        busyLabel="Deleting…"
        busy={deleting}
      />
    </Section>
  );
}
