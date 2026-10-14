import { Section } from "@/components/settings/shared/section";
import { modelSize } from "@/components/settings/transcription/LocalWhisper";
import {
  alignerStatus,
  cancelAlignerDownload,
  deleteAligner,
  downloadAligner,
  onAlignerProgress,
} from "@/lib/studio/aligner";
import { PackDownloadRow, packState } from "./PackDownloadRow";
import { usePackDownload, type PackApi } from "./usePackDownload";

const MODEL_NAME = "wav2vec2 base (English)";

const ALIGNER: PackApi<Awaited<ReturnType<typeof alignerStatus>>> = {
  name: "Aligner",
  status: alignerStatus,
  download: downloadAligner,
  cancel: cancelAlignerDownload,
  remove: deleteAligner,
  onProgress: onAlignerProgress,
};

/**
 * The narration aligner's model, downloaded on request like a Local Whisper
 * model: Download, its progress and Cancel, then Delete. Narration cannot
 * time its words without it. The status is local; only Download goes online.
 */
export function WordTimingSection() {
  const pack = usePackDownload(ALIGNER);
  const { status, download } = pack;

  return (
    <Section
      title="Word timing"
      description="Sync scene animations to narrated words."
    >
      <PackDownloadRow
        name={MODEL_NAME}
        detail={
          status
            ? `${modelSize(status.bytes)} download · ${packState(status.status, download !== null)}`
            : " "
        }
        pack={pack}
        missing="Narration can't time its words until this is downloaded."
        deleteDescription="Narration can't time its words until it is downloaded again from here."
      />

      <p className="pt-1 text-[11px] leading-relaxed text-muted-foreground">
        facebook/wav2vec2-base-960h, Apache-2.0
      </p>
    </Section>
  );
}
