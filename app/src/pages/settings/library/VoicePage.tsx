import { NarrationVoiceSection } from "@/components/settings/voice/NarrationVoiceSection";
import { WordTimingSection } from "@/components/settings/voice/WordTimingSection";
import { Separator } from "@/components/ui/separator";

export default function SettingsVoicePage() {
  return (
    <>
      <NarrationVoiceSection />

      <Separator className="my-7" />

      <WordTimingSection />
    </>
  );
}
