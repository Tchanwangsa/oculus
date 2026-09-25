import { useEffect, useMemo, useState } from "react";
import { Plus } from "@phosphor-icons/react";

import { forgetProviders } from "@/lib/opencodeAuth";
import { useCatalogue } from "@/lib/opencodeCatalogue";
import { providerHealth, useBridgeHealth } from "@/hooks/useBridgeHealth";
import { Button } from "@/components/ui/button";
import { Section } from "@/pages/settings/section";
import { OpencodeCatalogDialog } from "./OpencodeCatalogDialog";

/**
 * A summary of the opencode providers and a button into
 * `OpencodeCatalogDialog`, which holds the work. Opening Settings must not
 * start a CLI: `opencode serve` starts on the dialog's first read, and this
 * summary reads only the stored catalogue. Nothing here makes a billed call
 * (see CLAUDE.md; free-model filters live in `lib/opencodeCatalogue.ts`).
 */
export function OpencodeProvidersSection() {
  const { health } = useBridgeHealth();
  const installed = providerHealth(health, "opencode");
  const { catalogue } = useCatalogue();
  const [open, setOpen] = useState(false);

  // A cached provider list is stale once opencode is gone.
  useEffect(() => {
    if (installed === "missing") forgetProviders();
  }, [installed]);

  // From the catalogue alone, so it can only count what was hidden, not what exists.
  const summary = useMemo(() => {
    if (!catalogue) return null;
    const models = Object.values(catalogue.providers).reduce(
      (n, p) => n + Object.values(p.models).filter((m) => m.hidden === true).length,
      0,
    );
    const providers = catalogue.hiddenProviders.length;
    const parts: string[] = [];
    if (providers > 0) {
      parts.push(`${providers} ${providers === 1 ? "provider" : "providers"} hidden`);
    }
    if (models > 0) parts.push(`${models} ${models === 1 ? "model" : "models"} hidden`);
    return parts.length > 0
      ? parts.join(" · ")
      : "Chat offers every model the signed-in providers list.";
  }, [catalogue]);

  const description =
    "Chat's opencode agent reaches whichever providers opencode is signed in to. Credentials are saved in opencode's own store on this machine, so they are shared with the opencode you run in a terminal.";

  if (installed === "missing") {
    return (
      <Section title="opencode providers" description={description}>
        <p className="py-2.5 text-xs text-muted-foreground">
          opencode is not installed, so there is nothing to sign in to. Install it and press{" "}
          <span className="text-foreground">Recheck</span> above.
        </p>
      </Section>
    );
  }

  return (
    <Section title="opencode providers" description={description}>
      <div className="flex items-center gap-3 py-1">
        <Button variant="outline" size="sm" onClick={() => setOpen(true)}>
          <Plus size={12} />
          Manage providers
        </Button>
        <span className="min-w-0 truncate text-xs text-muted-foreground tabular-nums">
          {summary ?? "Reading your picker settings…"}
        </span>
      </div>

      {open && (
        <OpencodeCatalogDialog onClose={() => setOpen(false)} />
      )}
    </Section>
  );
}
