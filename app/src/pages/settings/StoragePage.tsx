import { Separator } from "@/components/ui/separator";
import { StartOver } from "@/components/settings/StartOver";
import { StorageSection } from "@/components/settings/StorageSection";
import { Section } from "./section";

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
