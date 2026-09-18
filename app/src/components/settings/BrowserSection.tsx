import { useEffect } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Section } from "@/pages/settings/section";
import { SEARCH_ENGINES } from "@/lib/browser";
import {
  useBrowserPrefsStore,
  type OpenLinksIn,
} from "@/stores/browserPrefsStore";

/** One label/control line. `StatRow` next door is for read-only numbers; this
 *  is its writable twin, with room for a sentence under the label. */
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
    <div className="flex items-start justify-between gap-6 py-2.5">
      <div className="min-w-0">
        <div className="text-xs text-foreground">{label}</div>
        {hint && (
          <p className="mt-0.5 text-[11px] text-muted-foreground">{hint}</p>
        )}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

/**
 * The two things about the in-app browser that are a preference.
 *
 * Neither costs anything to change and neither can fail, so both are plain
 * `onValueChange` with nothing in front of them — the house rule is that a
 * dialog raised over a change that destroys nothing is how people learn to
 * click through the one that matters.
 *
 * The engine is one list shared with the new-tab page's browser door and with
 * ⌘K's "search the web" row, so all three go to the same place
 * (`app/src/lib/browser.ts`).
 */
export function BrowserSection() {
  const { engine, openLinksIn, loaded, load, setEngine, setOpenLinksIn } =
    useBrowserPrefsStore();

  // Normally loaded once at startup by `useBrowserTabs`; asked again here so
  // the page is right even if it is the first thing rendered after a reload.
  useEffect(() => {
    if (!loaded) void load().catch(() => {});
  }, [loaded, load]);

  return (
    <Section
      title="Browser"
      description="Links in Oculus open as tabs here, signed in to Canvas — this is what that browser does."
    >
      <div>
        <SettingRow
          label="Search engine"
          hint="Where a typed query goes, from the address bar and from ⌘K."
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
          hint="Oculus keeps you signed in to Canvas, Ed and Echo360; your default browser does not. ⌘-click always leaves for the default browser."
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
