import { CodeSection } from "@/components/settings/appearance/CodeSection";
import { ThemeSection } from "@/components/settings/appearance/ThemeSection";
import { Separator } from "@/components/ui/separator";

export default function SettingsAppearancePage() {
  return (
    <>
      <ThemeSection />
      <Separator className="my-7" />
      <CodeSection />
    </>
  );
}
