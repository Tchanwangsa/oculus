import { useEffect, useState } from "react";
import { useLocation } from "react-router-dom";

import { forgetProviders, type OpencodeProvider } from "@/lib/harness/opencodeAuth";
import type { SettingsJump } from "@/lib/search/settings";
import { providerHealth, useBridgeHealth } from "@/hooks/agents/useBridgeHealth";
import { useStoredState } from "@/hooks/ui/useStoredState";
import { ViewTabs } from "@/components/ui/table/ViewTabs";
import { OpencodeConnectDialog } from "@/components/settings/agents/OpencodeConnectDialog";
import { ModelsTable, type ModelsFocus } from "@/components/settings/agents/opencode/ModelsTable";
import { ProvidersTable } from "@/components/settings/agents/opencode/ProvidersTable";
import { useOpencode } from "@/components/settings/agents/opencode/useOpencode";
import { settingsSectionId } from "@/components/settings/shared/section";

type View = "providers" | "models";
const VIEW_KEY = "oculus-settings-opencode-view";

/** Titles double as the Settings search sections that pick a tab. */
const VIEWS = [
  { value: "providers", label: "Providers" },
  { value: "models", label: "Models" },
] as const satisfies ReadonlyArray<{ value: View; label: string }>;

/**
 * opencode's providers and models as two full tables: connect, disconnect or
 * hide a provider, and tick which models Chat's picker offers. Opening this
 * page is what starts `opencode serve` (Settings opens on another page); the
 * connect flow is a dialog over it. Nothing here makes a billed call (see
 * docs/harness.md; the offered gate lives in `lib/harness/opencodeCatalogue.ts`).
 */
export default function SettingsOpencodePage() {
  const location = useLocation();
  const { health } = useBridgeHealth();
  const installed = providerHealth(health, "opencode");
  const oc = useOpencode(health !== null && installed !== "missing");
  const [view, setView] = useStoredState<View>(VIEW_KEY, (stored) =>
    stored === "models" ? "models" : "providers",
  );
  const [focus, setFocus] = useState<ModelsFocus>({ provider: null, n: 0 });
  const [connecting, setConnecting] = useState<OpencodeProvider | null>(null);

  // A cached provider list is stale once opencode is gone.
  useEffect(() => {
    if (installed === "missing") forgetProviders();
  }, [installed]);

  // A Settings search result names the tab as its section.
  useEffect(() => {
    const section = (location.state as SettingsJump | null)?.section;
    const hit = VIEWS.find((v) => v.label === section);
    if (hit) setView(hit.value);
  }, [location, setView]);

  const showModels = (provider: string | null) => {
    setFocus((f) => ({ provider, n: f.n + 1 }));
    setView("models");
  };

  return (
    <div className="flex h-full flex-col">
      <div className="shrink-0 px-5 pt-6">
        <h2 className="text-[15px] font-semibold text-foreground">opencode</h2>
        <p className="mt-0.5 max-w-2xl text-xs text-muted-foreground">
          Chat's opencode agent reaches whichever providers opencode is signed in to, and offers
          every model they list unless you untick it. Credentials are saved in opencode's own store
          on this machine, so they are shared with the opencode you run in a terminal.
        </p>
      </div>

      {installed === "missing" ? (
        <p className="px-5 py-4 text-xs text-muted-foreground">
          opencode is not installed, so there is nothing to sign in to. Install it and press{" "}
          <span className="text-foreground">Recheck</span> on the Agents page.
        </p>
      ) : (
        <>
          <div className="flex shrink-0 items-end border-b border-border-subtle px-5 pt-3">
            <ViewTabs tabs={VIEWS} value={view} onChange={setView} />
          </div>

          {view === "providers" ? (
            <div id={settingsSectionId("Providers")} className="flex min-h-0 flex-1 flex-col">
              <ProvidersTable
                list={oc.list}
                loading={oc.loading}
                onRetry={() => void oc.loadList(false)}
                catalogue={oc.catalogue}
                models={oc.models}
                removing={oc.removing}
                error={oc.error}
                onModels={showModels}
                onDisconnect={(p) => void oc.disconnect(p)}
                onHide={(id, hidden) => void oc.setProviderHidden(id, hidden)}
                onConnect={setConnecting}
              />
            </div>
          ) : (
            <div id={settingsSectionId("Models")} className="flex min-h-0 flex-1 flex-col">
              <ModelsTable
                models={oc.models}
                providers={oc.list?.providers ?? null}
                catalogue={oc.catalogue}
                focus={focus}
                error={oc.error}
                onHidden={(ids, hidden) => void oc.setModelsHidden(ids, hidden)}
                onShowProviders={() => setView("providers")}
              />
            </div>
          )}
        </>
      )}

      {connecting && (
        <OpencodeConnectDialog
          provider={connecting}
          onClose={() => setConnecting(null)}
          onDone={(next) => {
            const id = connecting.id;
            setConnecting(null);
            // Re-read the catalogue, which predates this provider, and land
            // on its models.
            oc.connected(next);
            showModels(id);
          }}
        />
      )}
    </div>
  );
}
