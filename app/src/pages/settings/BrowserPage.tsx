import { Separator } from "@/components/ui/separator";
import { BrowserSection } from "@/components/settings/BrowserSection";
import { BrowserHistorySection } from "@/components/settings/BrowserHistorySection";

export default function SettingsBrowserPage() {
  return (
    <>
      <BrowserSection />
      <Separator className="my-7" />
      <BrowserHistorySection />
    </>
  );
}
