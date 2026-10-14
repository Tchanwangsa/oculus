import { modelSize } from "@/components/settings/transcription/LocalWhisper";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { kokoroVoiceLabel, STARTER_VOICES } from "@/lib/studio/kokoroVoices";
import type { VoicePacksStatus } from "@/lib/studio/voicePacks";
import type { VoiceSettings } from "@/lib/studio/voice";
import { PackDownloadRow, packState } from "./PackDownloadRow";
import { SettingRow } from "./SettingRow";
import type { PackDownload } from "./usePackDownload";
import type { SaveVoice, VoiceErrors } from "./NarrationVoiceSection";
import { KokoroVoicesTable } from "./voices/KokoroVoicesTable";
import type { KokoroVoices } from "./voices/useKokoroVoices";

const PACK_NAMES: Record<VoicePacksStatus["packs"][number]["id"], string> = {
  "voice-kokoro": "Model",
  "g2p-en-us": "Pronunciation",
};

/**
 * The Kokoro engine's fields: the model and the pronunciation pack that
 * reads text to phonemes as one download, the voices (each its own small
 * download, with a sample), and the voice narration uses, chosen from the
 * ones downloaded.
 */
export function KokoroFields({ settings, errors, save, packs, files, speed }: {
  settings: VoiceSettings;
  errors: VoiceErrors;
  save: SaveVoice;
  packs: PackDownload<VoicePacksStatus>;
  files: KokoroVoices;
  speed: number;
}) {
  const { status, download } = packs;
  const downloaded = files.voices?.filter((v) => v.status === "present") ?? [];
  // A voice deleted since it was chosen, or one set for the server, stays in
  // the list so the select shows what is stored.
  const stored = settings.voice;
  const unlisted = stored !== null && files.voices !== null && !downloaded.some((v) => v.id === stored);
  const known = files.voices?.find((v) => v.id === stored);
  const starters = (files.voices ?? []).filter((v) => STARTER_VOICES.includes(v.id)).map((v) => v.name);

  return (
    <>
      <PackDownloadRow
        name="Model and pronunciation pack"
        detail={
          status ? (
            <>
              {status.packs.map((p) => `${PACK_NAMES[p.id]} ${modelSize(p.bytes)}`).join(" · ")}
              {" · "}
              {packState(status.status, download !== null)}
            </>
          ) : (
            " "
          )
        }
        pack={packs}
        downloadLabel="Download model and pronunciation pack"
        missing="Kokoro can't narrate until the model and pronunciation pack are downloaded."
        deleteDescription="Kokoro can't narrate until they are downloaded again from here. Voices stay."
      />

      <KokoroVoicesTable
        files={files}
        chosen={stored}
        speed={speed}
        canSample={status?.status === "present"}
      />

      <SettingRow
        label="Voice"
        hint={
          files.voices && downloaded.length === 0
            ? `Start with ${starters.join(" or ")}.`
            : "Choose a downloaded voice."
        }
        error={
          errors.voice ??
          (unlisted && stored
            ? known
              ? `${known.name} is not downloaded: download it above or choose one of these.`
              : `${stored} is not one of Kokoro's voices: choose one of these.`
            : null)
        }
      >
        <Select
          value={stored ?? ""}
          disabled={files.voices === null}
          onValueChange={(voice) => void save("voice", { voice })}
        >
          <SelectTrigger aria-label="Kokoro voice" size="sm" className="h-7 w-80 max-w-full text-xs">
            <SelectValue placeholder="Choose a voice" />
          </SelectTrigger>
          <SelectContent>
            {unlisted && stored ? (
              <SelectItem value={stored} className="text-xs">
                {known ? kokoroVoiceLabel(known) : stored}
              </SelectItem>
            ) : null}
            {downloaded.map((v) => (
              <SelectItem key={v.id} value={v.id} className="text-xs">
                {kokoroVoiceLabel(v)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </SettingRow>
    </>
  );
}
