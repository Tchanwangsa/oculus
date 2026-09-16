import { AppearanceSection } from "@/components/settings/AppearanceSection";
import { Section } from "./section";

export default function SettingsAppearancePage() {
  return (
    <Section title="Appearance" description="How Oculus looks on this machine.">
      <AppearanceSection />
    </Section>
  );
}
