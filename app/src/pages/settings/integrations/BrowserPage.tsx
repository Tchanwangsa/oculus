import { Separator } from "@/components/ui/separator";
import { BrowserSection } from "@/components/settings/web/BrowserSection";
import { BrowserHistorySection } from "@/components/settings/web/BrowserHistorySection";

export default function SettingsBrowserPage() {
  return (
    <>
      <BrowserSection />
      <Separator className="my-7" />
      <BrowserHistorySection />
    </>
  );
}
