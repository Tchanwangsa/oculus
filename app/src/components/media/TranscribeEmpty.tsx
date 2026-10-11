import { memo } from "react";
import { CircleNotch } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { PanelEmpty } from "@/components/media/MediaDock";
import { useTranscription, type TranscriptionRun } from "@/hooks/lectures/useTranscription";
import { ENGINE_LABELS } from "@/lib/lectures/transcribe";

export interface TranscribeEmptyProps {
  /** The video to transcribe, as `transcribe` takes it; null until it is on
   *  disk. */
  path: string | null;
  /** Runs once the VTT is written, before `TRANSCRIBED_EVENT` — a lecture
   *  records the path here. Keep it stable: this component is memoised. */
  after?: (vtt: string) => Promise<void>;
}

function runLabel(run: TranscriptionRun): string {
  if (run.phase === "extracting") return "Extracting the audio";
  if (run.phase === "done") return "Loading the transcript";
  const what = run.chunks > 1 ? `Transcribing part ${run.chunk} of ${run.chunks}` : "Transcribing";
  return run.engine ? `${what} with ${ENGINE_LABELS[run.engine]}` : what;
}

/**
 * The Transcript tab of a video without one: Transcribe, its progress, or why
 * it failed. Whoever shows the cues listens for `TRANSCRIBED_EVENT`.
 */
export const TranscribeEmpty = memo(function TranscribeEmpty({
  path,
  after,
}: TranscribeEmptyProps) {
  const { run, start } = useTranscription(path, after);

  return (
    <div className="flex-1 min-h-0 overflow-y-auto">
      <PanelEmpty>
        {!path ? (
          <>
            <p>No transcript for this recording.</p>
            <p className="text-muted-foreground">Download it to transcribe it.</p>
          </>
        ) : run && run.phase !== "error" ? (
          <span className="flex items-center gap-1.5 text-brand">
            <CircleNotch size={12} className="shrink-0 animate-spin" />
            <span className="tabular-nums">{runLabel(run)}</span>
          </span>
        ) : (
          <>
            {run?.error ? (
              <p className="text-destructive">{run.error}</p>
            ) : (
              <p>No transcript for this recording.</p>
            )}
            <Button size="xs" onClick={start}>
              {run?.error ? "Try again" : "Transcribe"}
            </Button>
          </>
        )}
      </PanelEmpty>
    </div>
  );
});
