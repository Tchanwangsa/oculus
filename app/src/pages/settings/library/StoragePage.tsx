import { StartOver } from "@/components/settings/library/StartOver";
import { StorageSection } from "@/components/settings/library/StorageSection";
import { Section } from "@/components/settings/shared/section";
import { Separator } from "@/components/ui/separator";

export default function SettingsStoragePage() {
  return (
    <>
      <Section
        title="Storage"
        description="Everything Oculus keeps on disk — downloads, parses, lecture videos, the search index."
      >
        <StorageSection />
      </Section>
      <Separator className="my-7" />
      <StartOver />
    </>
  );
}
