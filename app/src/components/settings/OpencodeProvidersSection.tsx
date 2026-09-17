import { useEffect, useMemo, useState } from "react";
import { Plus } from "@phosphor-icons/react";

import { forgetProviders } from "@/lib/opencodeAuth";
import { useCatalogue } from "@/lib/opencodeCatalogue";
import { providerHealth, useBridgeHealth } from "@/hooks/useBridgeHealth";
import { Button } from "@/components/ui/button";
import { Section } from "@/pages/settings/section";
import { OpencodeCatalogDialog } from "./OpencodeCatalogDialog";

/**
 * Which opencode providers this machine can reach, and how to change that —
 * as a summary and a button, with the whole surface in a dialog behind it
 * (`OpencodeCatalogDialog.tsx`).
 *
 * It used to be the surface. A connected list, a search box over 218
 * providers, and Connect/Disconnect sat inline on Settings → AI beside four
 * other sections — and a provider like OpenRouter contributes three hundred
 * models to untick. So the page keeps the one line a student reads on the way
 * past and the dialog holds the work.
 *
 * It also used to own a **sweep**: one billed request per model, to find out
 * which of them answer. That is deleted — see `opencodeCatalogue.ts` — and
 * with it the progress line that lived beside the button. Nothing on this page
 * or in the dialog behind it spends anything now.
 *
 * Two rules survive unchanged.
 *
 * **Opening Settings must not start a CLI.** Reading the provider list means
 * asking the app's `opencode serve`, and a settings page being opened is not a
 * reason to spawn one — the rule `harness_refresh_rate_limits` already keeps.
 * The click on *Manage providers* is what starts it, which now means the
 * dialog's first read rather than this section's. The summary above it costs
 * nothing at all: it is the stored catalogue, which is one `settings` row.
 *
 * **Whether opencode is installed is a free question.** `useBridgeHealth` has
 * already answered it, so a machine without opencode says so instead of
 * offering a button whose only outcome is an error.
 */
export function OpencodeProvidersSection() {
  const { health } = useBridgeHealth();
  const installed = providerHealth(health, "opencode");
  const { catalogue } = useCatalogue();
  const [open, setOpen] = useState(false);

  // A cached provider list from an earlier visit is only trustworthy while
  // opencode is the same install; a recheck that finds it gone must not let
  // the dialog open on 218 rows offering to sign in.
  useEffect(() => {
    if (installed === "missing") forgetProviders();
  }, [installed]);

  /**
   * Read from the catalogue alone — never from opencode, which is the rule
   * above. That is also its limit: the catalogue knows what the student has
   * **hidden** and nothing about how many models exist, since counting those
   * would mean starting a CLI. So the line says what was switched off, and
   * says nothing at all when nothing was.
   */
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
