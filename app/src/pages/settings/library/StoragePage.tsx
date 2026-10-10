import { StorageSection } from "@/components/settings/library/StorageSection";
import { Section } from "@/components/settings/shared/section";

export default function SettingsStoragePage() {
  return (
    <Section
      title="Storage"
      description="Everything Oculus keeps on disk — downloads, parses, lecture videos, the search index."
    >
      <StorageSection />
    </Section>
  );
}
