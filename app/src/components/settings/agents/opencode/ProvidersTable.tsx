import { useEffect, useMemo, useState } from "react";
import { CaretRight, CircleNotch, Eye, EyeSlash, Plugs } from "@phosphor-icons/react";

import type { OpencodeProvider, OpencodeProviderList } from "@/lib/harness/opencodeAuth";
import {
  filterOffered,
  isZenProvider,
  providerOf,
  type OpencodeCatalogue,
} from "@/lib/harness/opencodeCatalogue";
import type { HarnessModel } from "@/lib/harness";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { GridTable, HeaderLabels } from "@/components/ui/table/GridTable";
import { PillTabs } from "@/components/ui/table/PillTabs";
import { usePagedRows } from "@/components/ui/table/TablePagination";
import { IconAction, SearchField, TableEmpty } from "./parts";

/** Column template shared by the header and every row; the name takes the slack. */
const COLS =
  "grid grid-cols-[minmax(0,1fr)_110px_100px_130px_150px] items-center gap-3 px-5";

const PAGE_SIZE = 50;

type Scope = "connected" | "all";

/** opencode's `source`, for a connected provider. */
const SOURCE_LABEL: Record<string, string> = {
  api: "API key",
  env: "Environment",
  config: "opencode.json",
  custom: "Custom",
};

/**
 * Every provider opencode knows: connect, disconnect or hide, and a way into
 * its models. *Connected* holds the signed-in and hidden ones; *All* is the
 * whole list (~200), sign-in flows first. Search runs inside the scope.
 */
export function ProvidersTable({
  list,
  loading,
  onRetry,
  catalogue,
  models,
  removing,
  error,
  onModels,
  onDisconnect,
  onHide,
  onConnect,
}: {
  list: OpencodeProviderList | null;
  loading: boolean;
  onRetry: () => void;
  catalogue: OpencodeCatalogue | null;
  /** Every model opencode lists, or null before the read lands (counts only). */
  models: HarnessModel[] | null;
  removing: string | null;
  error: string | null;
  onModels: (providerId: string) => void;
  onDisconnect: (p: OpencodeProvider) => void;
  onHide: (providerId: string, hidden: boolean) => void;
  onConnect: (p: OpencodeProvider) => void;
}) {
  const [scope, setScope] = useState<Scope>("connected");
  const [query, setQuery] = useState("");

  const providers = list?.providers ?? [];
  const hiddenIds = useMemo(() => catalogue?.hiddenProviders ?? [], [catalogue]);
  const isHidden = (p: OpencodeProvider) => hiddenIds.includes(p.id);
  const isConnected = (p: OpencodeProvider) => p.connected || isHidden(p);

  const connectedCount = providers.filter((p) => p.connected && !hiddenIds.includes(p.id)).length;

  const rows = useMemo(() => {
    const hidden = new Set(hiddenIds);
    const rank = (p: OpencodeProvider) =>
      p.connected || hidden.has(p.id) ? 0 : p.methods.some((m) => m.kind === "oauth") ? 1 : 2;
    const q = query.trim().toLowerCase();
    return providers
      .filter((p) => scope === "all" || p.connected || hidden.has(p.id))
      .filter((p) => !q || p.name.toLowerCase().includes(q) || p.id.toLowerCase().includes(q))
      .sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name));
  }, [providers, hiddenIds, scope, query]);

  const { page, pageCount, setPage, pageRows } = usePagedRows(rows, PAGE_SIZE);
  useEffect(() => setPage(1), [scope, query, setPage]);

  /** Offered per provider, through `filterOffered`, the composer's own gate,
   *  so the counts agree with the picker. */
  const offered = useMemo(() => {
    if (!models || !catalogue) return null;
    const out = new Map<string, number>();
    for (const m of filterOffered(models, catalogue)) {
      const id = providerOf(m.id);
      out.set(id, (out.get(id) ?? 0) + 1);
    }
    return out;
  }, [models, catalogue]);

  const q = query.trim();
  const empty =
    list === null ? (
      <TableEmpty
        message={
          loading ? (
            <span className="inline-flex items-center gap-2">
              <CircleNotch size={12} className="animate-spin" />
              Starting opencode…
            </span>
          ) : (
            "opencode did not answer."
          )
        }
      >
        {!loading && (
          <Button variant="outline" size="sm" onClick={onRetry}>
            Try again
          </Button>
        )}
      </TableEmpty>
    ) : rows.length > 0 ? null : q && scope === "connected" ? (
      <TableEmpty message={`No connected provider matches “${q}”.`}>
        <Button variant="outline" size="sm" onClick={() => setScope("all")}>
          Search every provider
        </Button>
      </TableEmpty>
    ) : q ? (
      <TableEmpty message={`No provider called “${q}”.`}>
        <Button variant="outline" size="sm" onClick={() => setQuery("")}>
          Clear search
        </Button>
      </TableEmpty>
    ) : (
      <TableEmpty message="Nothing is signed in yet, so Chat's opencode agent has no models to run.">
        <Button variant="outline" size="sm" onClick={() => setScope("all")}>
          Add a provider
        </Button>
      </TableEmpty>
    );

  return (
    <>
      {/* Fixed height so switching tabs doesn't jump the table. */}
      <div className="flex h-12 shrink-0 items-center gap-3 px-5">
        <SearchField
          value={query}
          onChange={setQuery}
          placeholder={list ? `Search ${providers.length} providers` : "Search providers"}
          disabled={list === null}
        />
        <PillTabs
          tabs={[
            { value: "connected", label: "Connected" },
            { value: "all", label: "All" },
          ]}
          value={scope}
          onChange={setScope}
        />
        <span className="flex-1" />
        {list && (
          <span className="shrink-0 text-[11px] text-muted-foreground tabular-nums">
            {connectedCount} connected · {providers.length} providers
          </span>
        )}
      </div>

      {error && (
        <p data-selectable className="shrink-0 px-5 pb-2.5 text-xs text-destructive">
          {error}
        </p>
      )}

      <div className="min-h-0 flex-1">
        <GridTable
          cols={COLS}
          header={<HeaderLabels labels={["Provider", "Status", "Source", "Models", ""]} />}
          pagination={{ page, pageCount, onPage: setPage, total: rows.length, unit: "provider" }}
        >
          {empty ?? (
            <>
              <div className="divide-y divide-border-subtle border-b border-border-subtle">
                {pageRows.map((p) => (
                  <ProviderRow
                    key={p.id}
                    provider={p}
                    hidden={isHidden(p)}
                    connected={isConnected(p)}
                    offered={offered?.get(p.id) ?? (offered ? 0 : null)}
                    removing={removing === p.id}
                    onModels={onModels}
                    onDisconnect={onDisconnect}
                    onHide={onHide}
                    onConnect={onConnect}
                  />
                ))}
              </div>
              <div className="space-y-1.5 px-5 py-3 text-xs text-muted-foreground">
                <p>
                  {scope === "all"
                    ? "Providers with a sign-in flow come first. Every other provider takes an API key."
                    : "Add a provider under All: those with a sign-in flow open the browser, every other one takes an API key."}
                </p>
                {list?.stale && (
                  <p>
                    opencode is mid-turn, so this list is the one it read before. It catches up once
                    the turn finishes.
                  </p>
                )}
              </div>
            </>
          )}
        </GridTable>
      </div>
    </>
  );
}

function ProviderRow({
  provider,
  hidden,
  connected,
  offered,
  removing,
  onModels,
  onDisconnect,
  onHide,
  onConnect,
}: {
  provider: OpencodeProvider;
  hidden: boolean;
  /** Signed in, or hidden (a hidden provider is still configured). */
  connected: boolean;
  /** Offered models, or null until the catalogue and the ticks are read. */
  offered: number | null;
  removing: boolean;
  onModels: (providerId: string) => void;
  onDisconnect: (p: OpencodeProvider) => void;
  onHide: (providerId: string, hidden: boolean) => void;
  onConnect: (p: OpencodeProvider) => void;
}) {
  const n = provider.modelCount;
  const size = `${n} ${n === 1 ? "model" : "models"}`;
  const zen = isZenProvider(provider.id);
  const models = hidden
    ? "none offered"
    : !provider.connected || offered === null
      ? size
      : `${offered} of ${n} offered`;

  return (
    <div className={cn(COLS, "py-2 transition-colors hover:bg-surface/60")}>
      <div className="min-w-0">
        <div
          className={cn(
            "truncate text-xs",
            hidden ? "text-muted-foreground" : "text-foreground",
          )}
        >
          {provider.name}
        </div>
        {zen && provider.connected && (
          <div className="truncate text-[11px] text-muted-foreground">
            free tier — only runs inside opencode itself
          </div>
        )}
      </div>

      <div>
        {hidden ? (
          <Badge variant="secondary" className="text-[11px]">
            Hidden
          </Badge>
        ) : provider.connected ? (
          <Badge variant="success" className="text-[11px]">
            Connected
          </Badge>
        ) : (
          <Badge variant="outline" className="text-[11px] text-muted-foreground">
            Not connected
          </Badge>
        )}
      </div>

      <span className="truncate text-[11px] text-muted-foreground">
        {connected ? (SOURCE_LABEL[provider.source] ?? provider.source) : "—"}
      </span>

      <span className="truncate text-[11px] text-muted-foreground tabular-nums">{models}</span>

      <div className="flex items-center gap-1 justify-self-end">
        <ProviderActions
          provider={provider}
          hidden={hidden}
          removing={removing}
          onModels={onModels}
          onDisconnect={onDisconnect}
          onHide={onHide}
          onConnect={onConnect}
        />
      </div>
    </div>
  );
}

/** No per-model check — see docs/harness.md: no billed calls from Settings. */
function ProviderActions({
  provider,
  hidden,
  removing,
  onModels,
  onDisconnect,
  onHide,
  onConnect,
}: {
  provider: OpencodeProvider;
  hidden: boolean;
  removing: boolean;
  onModels: (providerId: string) => void;
  onDisconnect: (p: OpencodeProvider) => void;
  onHide: (providerId: string, hidden: boolean) => void;
  onConnect: (p: OpencodeProvider) => void;
}) {
  if (hidden) {
    return (
      <IconAction label="Show in Oculus" onClick={() => onHide(provider.id, false)}>
        <Eye size={12} />
      </IconAction>
    );
  }

  if (!provider.connected) {
    return (
      <Button variant="ghost" size="xs" onClick={() => onConnect(provider)}>
        Connect
      </Button>
    );
  }

  return (
    <>
      <Button variant="ghost" size="xs" onClick={() => onModels(provider.id)}>
        Models
        <CaretRight size={12} />
      </Button>
      {provider.source === "config" ? (
        // From the student's opencode.json: no credential to delete, so hide.
        <IconAction label="Hide from Oculus" onClick={() => onHide(provider.id, true)}>
          <EyeSlash size={12} />
        </IconAction>
      ) : (
        <IconAction label="Disconnect" disabled={removing} onClick={() => onDisconnect(provider)}>
          {removing ? <CircleNotch size={12} className="animate-spin" /> : <Plugs size={12} />}
        </IconAction>
      )}
    </>
  );
}
