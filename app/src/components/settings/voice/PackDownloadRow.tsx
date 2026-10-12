import { useState } from "react";
import type { ReactNode } from "react";
import { modelSize, percent } from "@/components/settings/transcription/LocalWhisper";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { Progress } from "@/components/ui/progress";
import type { AlignerState } from "@/lib/studio/aligner";
import type { PackDownload } from "./usePackDownload";

/** `Downloaded`, `Not downloaded`, or `Downloading` while `download` runs. */
export function packState(state: AlignerState | undefined, downloading: boolean): string {
  if (downloading || state === "downloading") return "Downloading";
  return state === "present" ? "Downloaded" : "Not downloaded";
}

/**
 * A downloadable model's row, like a Local Whisper model's: its name and
 * detail line, Download (then a percentage, a progress bar and Cancel) or
 * Delete behind a confirmation, and its error or `missing` warning beneath.
 */
export function PackDownloadRow<S extends { status: AlignerState; bytes: number }>({
  name,
  detail,
  pack,
  downloadLabel = "Download",
  missing,
  deleteDescription,
}: {
  name: string;
  /** The line under the name; a space until the status is read. */
  detail: ReactNode;
  pack: PackDownload<S>;
  /** The Download button's text. */
  downloadLabel?: string;
  /** Shown while it is not downloaded. */
  missing: string;
  /** The confirmation's text after "Frees N MB." */
  deleteDescription: string;
}) {
  const { status, download, error, start, cancel, remove } = pack;
  const [confirm, setConfirm] = useState(false);
  const [deleting, setDeleting] = useState(false);

  const bytes = status?.bytes ?? 0;
  const done = download ? percent(download) : 0;
  const present = status?.status === "present";

  const onDelete = async () => {
    setDeleting(true);
    await remove();
    setDeleting(false);
    setConfirm(false);
  };

  return (
    <div className="py-2">
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0">
          <p className="text-xs text-foreground">{name}</p>
          <div className="text-[11px] text-muted-foreground tabular-nums">{detail}</div>
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
              {downloadLabel}
            </Button>
          )}
        </div>
      </div>
      {download ? <Progress value={done} className="mt-1.5 h-1" indicatorClassName="bg-brand" /> : null}
      {error ? (
        <p className="mt-2 text-[11px] leading-relaxed text-destructive" data-selectable>
          {error}
        </p>
      ) : null}
      {status && !present && !download && !error ? (
        <p className="mt-2 text-[11px] leading-relaxed text-warning">{missing}</p>
      ) : null}

      <ConfirmDialog
        open={confirm}
        onCancel={() => setConfirm(false)}
        onConfirm={() => void onDelete()}
        title={`Delete ${name}?`}
        description={`Frees ${modelSize(bytes)}. ${deleteDescription}`}
        confirmLabel="Delete"
        busyLabel="Deleting…"
        busy={deleting}
      />
    </div>
  );
}
