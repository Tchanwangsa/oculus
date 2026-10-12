import { modelSize } from "@/components/settings/transcription/LocalWhisper";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { kokoroVoiceName, type VoicePacksStatus } from "@/lib/studio/voicePacks";
import type { VoiceSettings } from "@/lib/studio/voice";
import { PackDownloadRow, packState } from "./PackDownloadRow";
import { SettingRow } from "./SettingRow";
import type { PackDownload } from "./usePackDownload";
import type { SaveVoice, VoiceErrors } from "./NarrationVoiceSection";

const PACK_NAMES: Record<VoicePacksStatus["packs"][number]["id"], string> = {
  "voice-kokoro": "Voice model",
  "g2p-en-us": "Pronunciation",
};

/** `Heart · American, female`, or the bare id. */
function voiceLabel(id: string): string {
  const { name, detail } = kokoroVoiceName(id);
  return detail ? `${name} · ${detail}` : name;
}

/**
 * The Kokoro engine's fields: its two packs as one download (the model and
 * voices, and the G2P that reads text to phonemes), and the voice, from the
 * pack's and any other voice file in the folder.
 */
export function KokoroFields({ settings, errors, save, packs }: {
  settings: VoiceSettings;
  errors: VoiceErrors;
  save: SaveVoice;
  packs: PackDownload<VoicePacksStatus>;
}) {
  const { status, download } = packs;
  const voices = status?.voices ?? [];
  // A voice set for the server, or a file since removed, stays in the list
  // so the select shows what is stored.
  const stored = settings.voice;
  const unknown = stored !== null && status !== null && !voices.some((v) => v.id === stored);

  return (
    <>
      <PackDownloadRow
        name="Kokoro voice packs"
        detail={
          status ? (
            <>
              {status.packs.map((p) => `${PACK_NAMES[p.id]} ${modelSize(p.bytes)}`).join(" · ")}
              {" · "}
              {packState(status.status, download !== null)}
            </>
          ) : (
            " "
          )
        }
        pack={packs}
        missing="Kokoro can't narrate until both packs are downloaded."
        deleteDescription="Kokoro can't narrate until they are downloaded again from here. Other voice files in the folder stay."
      />

      <SettingRow
        label="Voice"
        hint="Kokoro's US English voices."
        error={
          errors.voice ??
          (unknown ? `${stored} is not a Kokoro voice in the voice folder: choose one of these.` : null)
        }
      >
        <Select
          value={stored ?? ""}
          disabled={status === null}
          onValueChange={(voice) => void save("voice", { voice })}
        >
          <SelectTrigger aria-label="Kokoro voice" size="sm" className="h-7 w-80 text-xs">
            <SelectValue placeholder="Choose a voice" />
          </SelectTrigger>
          <SelectContent>
            {unknown && stored ? (
              <SelectItem value={stored} className="text-xs">
                {stored}
              </SelectItem>
            ) : null}
            {voices.map((v) => (
              <SelectItem key={v.id} value={v.id} className="text-xs">
                {voiceLabel(v.id)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </SettingRow>
    </>
  );
}
