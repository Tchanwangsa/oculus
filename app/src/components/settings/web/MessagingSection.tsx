import { useEffect } from "react";
import { Switch } from "@/components/ui/switch";
import { Section } from "@/components/settings/shared/section";
import { DOCK_SERVICES } from "@/lib/dock";
import { useDockPrefsStore } from "@/stores/shell/dockPrefsStore";

/**
 * One toggle per messaging service. On opens the service's page and adds it
 * to the rail; off closes the page, keeping its sign-in for next time.
 */
export function MessagingSection() {
  const { enabled, loaded, load, setEnabled } = useDockPrefsStore();

  // Also loaded by `useDockPages` at startup; re-read in case this renders first.
  useEffect(() => {
    if (!loaded) void load().catch(() => {});
  }, [loaded, load]);

  return (
    <Section
      title="Messaging"
      description="Each service you turn on gets a place in the sidebar and keeps running while you work elsewhere."
    >
      <div>
        {DOCK_SERVICES.map((service) => (
          <div key={service.id} className="flex items-center gap-3 py-2.5">
            <service.icon size={16} className="shrink-0 text-muted-foreground" />
            <div className="min-w-0 flex-1">
              <div className="text-xs text-foreground">{service.label}</div>
              <p className="mt-0.5 text-[11px] text-muted-foreground">{service.description}</p>
            </div>
            <Switch
              aria-label={`Show ${service.label}`}
              checked={enabled.includes(service.id)}
              disabled={!loaded}
              onCheckedChange={(on) => void setEnabled(service.id, on).catch(() => {})}
              className="shrink-0"
            />
          </div>
        ))}
      </div>
    </Section>
  );
}
