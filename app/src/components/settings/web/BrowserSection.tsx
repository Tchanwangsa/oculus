import { shortcut } from "@/lib/platform";
import { useEffect } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Section } from "@/components/settings/shared/section";
import { SEARCH_ENGINES } from "@/lib/browser";
import {
  useBrowserPrefsStore,
  type OpenLinksIn,
} from "@/stores/shell/browserPrefsStore";

/** A label/control line — the writable twin of `StatRow`. */
function SettingRow({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="settings-row flex items-start justify-between gap-6 py-2.5">
      <div className="min-w-0">
        <div className="text-xs text-foreground">{label}</div>
        {hint && (
          <p className="mt-0.5 text-[11px] text-muted-foreground">{hint}</p>
        )}
      </div>
      <div className="settings-controls shrink-0">{children}</div>
    </div>
  );
}

/**
 * In-app browser preferences. The search engine list is shared with the
 * new-tab page and ⌘K's web search (`app/src/lib/browser/index.ts`).
 */
export function BrowserSection() {
  const { engine, openLinksIn, loaded, load, setEngine, setOpenLinksIn } =
    useBrowserPrefsStore();

  // Also loaded by `useBrowserTabs` at startup; re-read in case this renders first.
  useEffect(() => {
    if (!loaded) void load().catch(() => {});
  }, [loaded, load]);

  return (
    <Section
      title="Browser"
      description="Search and link preferences."
    >
      <div>
        <SettingRow
          label="Search engine"
          hint={`Address bar and ${shortcut("mod+K")} searches.`}
        >
          <Select value={engine} onValueChange={(id) => void setEngine(id)}>
            <SelectTrigger aria-label="Search engine" size="sm" className="h-7 w-48 text-xs">
              <SelectValue placeholder="—" />
            </SelectTrigger>
            <SelectContent>
              {SEARCH_ENGINES.map((option) => (
                <SelectItem key={option.id} value={option.id} className="text-xs">
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </SettingRow>

        <SettingRow
          label="Open links in"
          hint={`${shortcut("mod")}+click uses your default browser.`}
        >
          <Select
            value={openLinksIn}
            onValueChange={(where) => void setOpenLinksIn(where as OpenLinksIn)}
          >
            <SelectTrigger aria-label="Open links in" size="sm" className="h-7 w-48 text-xs">
              <SelectValue placeholder="—" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="oculus" className="text-xs">
                Oculus
              </SelectItem>
              <SelectItem value="system" className="text-xs">
                Default browser
              </SelectItem>
            </SelectContent>
          </Select>
        </SettingRow>
      </div>
    </Section>
  );
}
