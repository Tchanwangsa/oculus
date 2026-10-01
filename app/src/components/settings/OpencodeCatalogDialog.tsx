import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  CaretLeft,
  CaretRight,
  CircleNotch,
  Eye,
  EyeSlash,
  MagnifyingGlass,
  Plugs,
  X,
} from "@phosphor-icons/react";

import {
  cachedProviders,
  opencodeDisconnect,
  opencodeProviders,
  rememberProviders,
  type OpencodeProvider,
  type OpencodeProviderList,
} from "@/lib/opencodeAuth";
import {
  forgetProvider,
  providerOf,
  filterOffered,
  isZen,
  isZenProvider,
  unusableReason,
  useCatalogue,
  type ModelEntry,
  type OpencodeCatalogue,
} from "@/lib/opencodeCatalogue";
import {
  harnessOpencodeModels,
  opencodeAsModels,
  type HarnessModel,
} from "@/lib/harness";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { PillTabs } from "@/components/ui/PillTabs";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { OpencodeConnectDialog } from "./OpencodeConnectDialog";

/** Cap on provider search results, so a one-letter query isn't every row. */
const RESULTS = 30;

type View = { kind: "providers" } | { kind: "models"; providerId: string };

/**
 * opencode's providers in one dialog: connect, disconnect or hide, and tick
 * which models the composer offers. Two views in local state (`providers` →
 * `models:<id>`); the connect flow is its own dialog on top.
 *
 * Mounting is what starts `opencode serve` — Settings only draws a button.
 * A `config`-sourced provider comes from the user's own `opencode.json`, which
 * Oculus never edits, so it can only be hidden (`hiddenProviders`).
 * Model rows come from `harnessOpencodeModels()` (the unfiltered catalogue),
 * never the already-filtered picker list, or a hidden model could never be
 * unhidden.
 */
export function OpencodeCatalogDialog({ onClose }: { onClose: () => void }) {
  const [view, setView] = useState<View>({ kind: "providers" });
  const [list, setList] = useState<OpencodeProviderList | null>(cachedProviders);
  const [loading, setLoading] = useState(() => !cachedProviders());
  const [error, setError] = useState<string | null>(null);
  const [removing, setRemoving] = useState<string | null>(null);
  const [connecting, setConnecting] = useState<OpencodeProvider | null>(null);
  const [query, setQuery] = useState("");
  const [modelQuery, setModelQuery] = useState("");

  const { catalogue, save } = useCatalogue();

  const loadList = useCallback(async (refresh: boolean) => {
    setLoading(true);
    setError(null);
    try {
      setList(rememberProviders(await opencodeProviders(refresh)));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!cachedProviders()) void loadList(false);
  }, [loadList]);

  /** opencode's whole catalogue (every provider, one call), held for the
   *  dialog's life; the ref dedupes loads without a stale closure. */
  const modelsRef = useRef<HarnessModel[] | null>(null);
  const inFlight = useRef<Promise<HarnessModel[]> | null>(null);
  const [models, setModels] = useState<HarnessModel[] | null>(null);

  const loadModels = useCallback(async (force: boolean): Promise<HarnessModel[]> => {
    if (!force && modelsRef.current) return modelsRef.current;
    if (!force && inFlight.current) return inFlight.current;
    const p = harnessOpencodeModels()
      .then((raw) => {
        const next = opencodeAsModels(raw);
        modelsRef.current = next;
        setModels(next);
        inFlight.current = null;
        return next;
      })
      .catch((e) => {
        inFlight.current = null;
        setError(String(e));
        return [] as HarnessModel[];
      });
    inFlight.current = p;
    return p;
  }, []);

  // Read on open: the provider rows' offered counts need it.
  useEffect(() => {
    void loadModels(false);
  }, [loadModels]);

  const openModels = (providerId: string) => {
    setModelQuery("");
    setView({ kind: "models", providerId });
    void loadModels(false);
  };

  const disconnect = async (provider: OpencodeProvider) => {
    setRemoving(provider.id);
    setError(null);
    try {
      setList(rememberProviders(await opencodeDisconnect(provider.id)));
      // Reconnecting later starts from a clean catalogue entry.
      if (catalogue) await save(forgetProvider(catalogue, provider.id));
    } catch (e) {
      setError(String(e));
    } finally {
      setRemoving(null);
    }
  };

  const setProviderHidden = async (providerId: string, hidden: boolean) => {
    if (!catalogue) return;
    const rest = catalogue.hiddenProviders.filter((p) => p !== providerId);
    await save({
      ...catalogue,
      hiddenProviders: hidden ? [...rest, providerId] : rest,
    });
  };

  const providers = list?.providers ?? [];
  const hiddenIds = catalogue?.hiddenProviders ?? [];

  // A hidden provider is listed only under *Hidden*.
  const connected = useMemo(
    () => providers.filter((p) => p.connected && !hiddenIds.includes(p.id)),
    [providers, hiddenIds],
  );
  const hidden = useMemo(
    () => providers.filter((p) => hiddenIds.includes(p.id)),
    [providers, hiddenIds],
  );
  /** Providers with an OAuth flow; the rest are plain API keys, found by search. */
  const featured = useMemo(
    () => providers.filter((p) => !p.connected && p.methods.some((m) => m.kind === "oauth")),
    [providers],
  );
  const matches = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return [];
    return providers.filter(
      (p) => p.name.toLowerCase().includes(q) || p.id.toLowerCase().includes(q),
    );
  }, [providers, query]);
  const results = useMemo(() => matches.slice(0, RESULTS), [matches]);

  const shown = view.kind === "models" ? providers.find((p) => p.id === view.providerId) : undefined;
  const providerModels = useMemo(() => {
    if (view.kind !== "models") return [];
    return (models ?? []).filter((m) => providerOf(m.id) === view.providerId);
  }, [models, view]);
  const filteredModels = useMemo(() => {
    const q = modelQuery.trim().toLowerCase();
    if (!q) return providerModels;
    return providerModels.filter(
      (m) => m.label.toLowerCase().includes(q) || m.id.toLowerCase().includes(q),
    );
  }, [providerModels, modelQuery]);

  const setHidden = async (ids: string[], hide: boolean) => {
    if (!catalogue || view.kind !== "models") return;
    await save(withHidden(catalogue, view.providerId, ids, hide));
  };

  const title = view.kind === "models" ? (shown?.name ?? view.providerId) : "opencode providers";

  return (
    <>
      <Dialog open onOpenChange={(open) => !open && onClose()}>
        <DialogContent className="flex h-[min(80vh,42rem)] flex-col gap-3 sm:max-w-2xl">
          <DialogHeader className="shrink-0">
            <div className="flex items-center gap-1.5">
              {view.kind === "models" && (
                <Button
                  variant="ghost"
                  size="icon-xs"
                  aria-label="Back to providers"
                  onClick={() => setView({ kind: "providers" })}
                >
                  <CaretLeft size={12} />
                </Button>
              )}
              <DialogTitle>{title}</DialogTitle>
            </div>
            <DialogDescription>
              {view.kind === "models"
                ? "Chat offers every model opencode lists. Untick the ones you do not want in the picker."
                : "Credentials are saved in opencode's own store on this machine, so they are shared with the opencode you run in a terminal."}
            </DialogDescription>
          </DialogHeader>

          {view.kind === "providers" ? (
            <ProvidersView
              loading={loading}
              onRetry={() => void loadList(false)}
              list={list}
              catalogue={catalogue}
              connected={connected}
              hidden={hidden}
              featured={featured}
              results={results}
              matched={matches.length}
              query={query}
              onQuery={setQuery}
              models={models}
              removing={removing}
              onModels={openModels}
              onDisconnect={(p) => void disconnect(p)}
              onHide={(id, h) => void setProviderHidden(id, h)}
              onConnect={setConnecting}
            />
          ) : (
            <ModelsView
              provider={shown}
              providerId={view.providerId}
              onBack={() => setView({ kind: "providers" })}
              models={providerModels}
              filtered={filteredModels}
              loaded={models !== null}
              catalogue={catalogue}
              query={modelQuery}
              onQuery={setModelQuery}
              onHidden={(ids, hide) => void setHidden(ids, hide)}
            />
          )}

          {error && <p className="shrink-0 text-xs text-destructive">{error}</p>}

          <DialogFooter className="shrink-0">
            <Button variant="outline" size="sm" onClick={onClose}>
              Close
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {connecting && (
        <OpencodeConnectDialog
          provider={connecting}
          onClose={() => setConnecting(null)}
          onDone={(next) => {
            const id = connecting.id;
            setList(rememberProviders(next));
            setConnecting(null);
            setQuery("");
            // The held catalogue predates this provider: re-read it and land
            // on its model list.
            setModelQuery("");
            setView({ kind: "models", providerId: id });
            void loadModels(true);
          }}
        />
      )}
    </>
  );
}

// ── the provider list ──────────────────────────────────────────────────────

/**
 * Connected, hidden and addable providers under a pinned search, in one scroll
 * region. The query is the mode: empty shows the groups, typing replaces the
 * whole body with matches, so neither starves the other of height.
 */
function ProvidersView({
  loading,
  onRetry,
  list,
  catalogue,
  connected,
  hidden,
  featured,
  results,
  matched,
  query,
  onQuery,
  models,
  removing,
  onModels,
  onDisconnect,
  onHide,
  onConnect,
}: {
  loading: boolean;
  onRetry: () => void;
  list: OpencodeProviderList | null;
  catalogue: OpencodeCatalogue | null;
  connected: OpencodeProvider[];
  hidden: OpencodeProvider[];
  featured: OpencodeProvider[];
  results: OpencodeProvider[];
  /** How many providers the query matched before `RESULTS` cut it. */
  matched: number;
  query: string;
  onQuery: (q: string) => void;
  /** Every model opencode lists, or null before the read lands (counts only). */
  models: HarnessModel[] | null;
  removing: string | null;
  onModels: (providerId: string) => void;
  onDisconnect: (p: OpencodeProvider) => void;
  onHide: (providerId: string, hidden: boolean) => void;
  onConnect: (p: OpencodeProvider) => void;
}) {
  if (list === null) {
    // On failure the error shows below; offer a retry, not a spinner.
    return (
      <div className="flex min-h-0 flex-1 items-center gap-2 text-xs text-muted-foreground">
        {loading ? (
          <>
            <CircleNotch size={12} className="animate-spin" />
            <span>Starting opencode…</span>
          </>
        ) : (
          <Button variant="outline" size="sm" onClick={onRetry}>
            Try again
          </Button>
        )}
      </div>
    );
  }

  const searching = query.trim().length > 0;
  const actions = (p: OpencodeProvider) => (
    <ProviderActions
      provider={p}
      removing={removing === p.id}
      onModels={onModels}
      onDisconnect={onDisconnect}
      onHide={onHide}
      onConnect={onConnect}
    />
  );

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="relative shrink-0">
        <MagnifyingGlass
          size={13}
          className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-muted-foreground"
        />
        <Input
          value={query}
          onChange={(e) => onQuery(e.target.value)}
          placeholder={`Search ${list.providers.length} providers`}
          className="px-8"
        />
        {searching && (
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label="Clear search"
            className="absolute top-1/2 right-1.5 -translate-y-1/2"
            onClick={() => onQuery("")}
          >
            <X size={12} />
          </Button>
        )}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto pr-1">
        {searching ? (
          results.length === 0 ? (
            <Empty message={`No provider called “${query.trim()}”.`}>
              <Button variant="outline" size="sm" onClick={() => onQuery("")}>
                Clear search
              </Button>
            </Empty>
          ) : (
            <div className="divide-y divide-border-subtle">
              {results.map((p) => (
                <ProviderRow
                  key={p.id}
                  provider={p}
                  catalogue={catalogue}
                  models={models}
                >
                  {actions(p)}
                </ProviderRow>
              ))}
              {matched > results.length && (
                <p className="py-2.5 text-xs text-muted-foreground tabular-nums">
                  {results.length} of {matched} matches — keep typing to narrow it.
                </p>
              )}
            </div>
          )
        ) : (
          <div className="flex flex-col gap-4">
            <div>
              <GroupLabel>Connected</GroupLabel>
              <div className="divide-y divide-border-subtle">
                {connected.map((p) => (
                  <ProviderRow
                    key={p.id}
                    provider={p}
                    catalogue={catalogue}
                    models={models}
                  >
                    {actions(p)}
                  </ProviderRow>
                ))}
                {connected.length === 0 && (
                  <p className="py-2.5 text-xs text-muted-foreground">
                    Nothing is signed in yet, so Chat's opencode agent has no models to run.
                  </p>
                )}
              </div>
            </div>

            {hidden.length > 0 && (
              <div>
                <GroupLabel>Hidden</GroupLabel>
                <div className="divide-y divide-border-subtle">
                  {hidden.map((p) => (
                    <ProviderRow
                      key={p.id}
                      provider={p}
                      catalogue={catalogue}
                      models={models}
                      muted
                      hiddenNote="none of its models reach the picker"
                    >
                      <IconAction label="Show in Oculus" onClick={() => onHide(p.id, false)}>
                        <Eye size={12} />
                      </IconAction>
                    </ProviderRow>
                  ))}
                </div>
              </div>
            )}

            <div>
              <GroupLabel>Add a provider</GroupLabel>
              <div className="divide-y divide-border-subtle">
                {featured.map((p) => (
                  <ProviderRow key={p.id} provider={p} catalogue={catalogue} models={models}>
                    {actions(p)}
                  </ProviderRow>
                ))}
                <p className="py-2.5 text-xs text-muted-foreground">
                  {featured.length > 0
                    ? "These offer a sign-in flow. Every other provider takes an API key — search for it by name above."
                    : "Search for a provider by name above to add an API key."}
                </p>
              </div>
            </div>

            {list.stale && (
              <p className="text-xs text-muted-foreground">
                opencode is mid-turn, so this list is the one it read before. It catches up once the
                turn finishes.
              </p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

/** A provider row's actions, shared by search results and the groups so both
 *  offer the same thing. No per-model check — see docs/harness.md: no billed
 *  calls from Settings. */
function ProviderActions({
  provider,
  removing,
  onModels,
  onDisconnect,
  onHide,
  onConnect,
}: {
  provider: OpencodeProvider;
  removing: boolean;
  onModels: (providerId: string) => void;
  onDisconnect: (p: OpencodeProvider) => void;
  onHide: (providerId: string, hidden: boolean) => void;
  onConnect: (p: OpencodeProvider) => void;
}) {
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
        // From the user's opencode.json: no credential to delete, so hide.
        <IconAction label="Hide from Oculus" onClick={() => onHide(provider.id, true)}>
          <EyeSlash size={12} />
        </IconAction>
      ) : (
        <IconAction
          label="Disconnect"
          disabled={removing}
          onClick={() => onDisconnect(provider)}
        >
          {removing ? (
            <CircleNotch size={12} className="animate-spin" />
          ) : (
            <Plugs size={12} />
          )}
        </IconAction>
      )}
    </>
  );
}

/** A ghost icon button whose tooltip is its accessible name. */
function IconAction({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label={label}
          disabled={disabled}
          onClick={onClick}
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

function ProviderRow({
  provider,
  catalogue,
  models,
  muted,
  hiddenNote,
  children,
}: {
  provider: OpencodeProvider;
  catalogue: OpencodeCatalogue | null;
  models: HarnessModel[] | null;
  /** Greys the name (the *Hidden* group). */
  muted?: boolean;
  /** Replaces the offered count in the *Hidden* group. */
  hiddenNote?: string;
  children: React.ReactNode;
}) {
  // Offered count for a connected provider; plain size until models load.
  const mine = models?.filter((m) => providerOf(m.id) === provider.id) ?? null;
  // `filterOffered`, the composer's own gate, so the counts agree.
  const offered = mine && catalogue ? filterOffered(mine, catalogue).length : null;
  const size = `${provider.modelCount} ${provider.modelCount === 1 ? "model" : "models"}`;
  const line = hiddenNote
    ? hiddenNote
    : !provider.connected
      ? size
      : isZenProvider(provider.id)
        ? "free tier — only runs inside opencode itself"
        : offered === null
          ? size
          : `${offered} of ${provider.modelCount} models offered`;

  return (
    <div className="flex items-center justify-between gap-4 py-2">
      <div className="min-w-0">
        <div
          className={cn(
            "truncate text-[13px]",
            muted ? "text-muted-foreground" : "text-foreground",
          )}
        >
          {provider.name}
        </div>
        <div className="mt-0.5 flex items-center gap-1.5 truncate text-xs text-muted-foreground tabular-nums">
          <span className="truncate">{line}</span>
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-1">{children}</div>
    </div>
  );
}

// ── one provider's models ──────────────────────────────────────────────────

function ModelsView({
  provider,
  providerId,
  onBack,
  models,
  filtered: searched,
  loaded,
  catalogue,
  query,
  onQuery,
  onHidden,
}: {
  provider: OpencodeProvider | undefined;
  providerId: string;
  onBack: () => void;
  models: HarnessModel[];
  filtered: HarnessModel[];
  loaded: boolean;
  catalogue: OpencodeCatalogue | null;
  query: string;
  onQuery: (q: string) => void;
  onHidden: (ids: string[], hidden: boolean) => void;
}) {
  const entry = catalogue?.providers[providerId];

  // *Selected* snapshots the ticks when chosen, not live, so unticking a row
  // doesn't pull it out from under the click.
  const [picked, setPicked] = useState<Set<string> | null>(null);
  const scope = picked ? "selected" : "all";
  const onScope = (next: "all" | "selected") =>
    setPicked(
      next === "all"
        ? null
        : new Set(
            models
              .filter((m) => blockedBecause(m) === null && entry?.models[m.id]?.hidden !== true)
              .map((m) => m.id),
          ),
    );
  const filtered = picked ? searched.filter((m) => picked.has(m.id)) : searched;

  // Blocked rows stay listed with their reason but are never counted offered.
  const blocked = models.filter((m) => blockedBecause(m) !== null).length;
  const usable = models.filter((m) => blockedBecause(m) === null);
  const hidden = usable.filter((m) => entry?.models[m.id]?.hidden === true).length;
  const offered = Math.max(models.length - blocked - hidden, 0);
  const status =
    blocked > 0
      ? `${offered} of ${models.length} offered · ${blocked} cannot run here`
      : `${offered} of ${models.length} offered`;

  // Bulk ticks act on the filtered rows only.
  const tickable = filtered.filter((m) => blockedBecause(m) === null);
  const showable = tickable.filter((m) => entry?.models[m.id]?.hidden === true).map((m) => m.id);
  const hideable = tickable.filter((m) => entry?.models[m.id]?.hidden !== true);

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      <div className="flex shrink-0 items-center gap-2">
        <div className="relative min-w-0 flex-1">
          <MagnifyingGlass
            size={13}
            className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-muted-foreground"
          />
          <Input
            value={query}
            onChange={(e) => onQuery(e.target.value)}
            placeholder={`Search ${models.length} models`}
            className="pl-8"
          />
        </div>
        <PillTabs
          tabs={[
            { value: "all", label: "All" },
            { value: "selected", label: "Selected" },
          ]}
          value={scope}
          onChange={onScope}
        />
      </div>

      <div className="flex shrink-0 items-center justify-between gap-3">
        <span className="min-w-0 truncate text-xs text-muted-foreground tabular-nums">{status}</span>
        <div className="flex shrink-0 items-center gap-1">
          <Button
            variant="ghost"
            size="xs"
            disabled={showable.length === 0}
            onClick={() => onHidden(showable, false)}
          >
            Show all
          </Button>
          <Button
            variant="ghost"
            size="xs"
            disabled={hideable.length === 0}
            onClick={() => onHidden(hideable.map((m) => m.id), true)}
          >
            Hide all
          </Button>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto pr-1">
        {!loaded ? (
          <div className="flex items-center gap-2 py-2.5 text-xs text-muted-foreground">
            <CircleNotch size={12} className="animate-spin" />
            <span>Reading opencode's catalogue…</span>
          </div>
        ) : filtered.length === 0 ? (
          <Empty
            message={
              picked && query.trim()
                ? `No ticked model matches “${query.trim()}”.`
                : picked
                  ? "No model here is ticked."
                  : query.trim()
                    ? `No model here matches “${query.trim()}”.`
                    : provider
                      ? `opencode lists no models for ${provider.name}.`
                      : "opencode lists no models for this provider."
            }
          >
            {query.trim() ? (
              <Button variant="outline" size="sm" onClick={() => onQuery("")}>
                Clear search
              </Button>
            ) : picked ? (
              <Button variant="outline" size="sm" onClick={() => onScope("all")}>
                Show every model
              </Button>
            ) : (
              <Button variant="outline" size="sm" onClick={onBack}>
                <CaretLeft size={12} />
                Back to providers
              </Button>
            )}
          </Empty>
        ) : (
          <div className="divide-y divide-border-subtle">
            {filtered.map((m) => (
              <ModelRow
                key={m.id}
                model={m}
                stored={entry?.models[m.id]}
                onHidden={(hide) => onHidden([m.id], hide)}
              />
            ))}
          </div>
        )}
      </div>

    </div>
  );
}

function ModelRow({
  model,
  stored,
  onHidden,
}: {
  model: HarnessModel;
  stored: ModelEntry | undefined;
  onHidden: (hidden: boolean) => void;
}) {
  const hidden = stored?.hidden === true;

  // A model the gate never offers can't be ticked.
  const why = blockedBecause(model);
  const blocked = why !== null;

  // The whole row is the checkbox. Not a `<label>`: Radix's Checkbox is a
  // `button`, and label activation would toggle it twice.
  return (
    <div
      role="checkbox"
      aria-checked={!blocked && !hidden}
      aria-disabled={blocked}
      aria-label={model.label}
      tabIndex={blocked ? -1 : 0}
      onClick={() => !blocked && onHidden(!hidden)}
      onKeyDown={(e) => {
        if (blocked || (e.key !== " " && e.key !== "Enter")) return;
        e.preventDefault();
        onHidden(!hidden);
      }}
      className="flex cursor-default items-start gap-2.5 rounded-md py-2 outline-none focus-visible:bg-accent"
    >
      <Checkbox
        tabIndex={-1}
        className="pointer-events-none mt-0.5"
        checked={!blocked && !hidden}
        disabled={blocked}
      />
      <div className="min-w-0 flex-1">
        <div className="truncate text-[13px] text-foreground">{model.label}</div>
        <div className="mt-0.5 truncate text-xs text-muted-foreground">{model.id}</div>
        {why && <div className="mt-0.5 text-xs text-muted-foreground">{why}</div>}
      </div>
    </div>
  );
}

/** An empty list with its way out (back, or clear the query) centred in it. */
function Empty({ message, children }: { message: string; children: React.ReactNode }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 py-10 text-center">
      <p className="max-w-xs text-xs text-muted-foreground">{message}</p>
      {children}
    </div>
  );
}

function GroupLabel({ children }: { children: React.ReactNode }) {
  return <div className="mb-1 text-xs font-medium text-muted-foreground">{children}</div>;
}

// ── catalogue arithmetic ───────────────────────────────────────────────────

/** Why this model is never offered, or null (see `lib/opencodeCatalogue.ts`).
 *  Zen first: those models claim every capability and still refuse. */
function blockedBecause(model: HarnessModel): string | null {
  if (isZen(model.id)) {
    return "opencode’s free tier only runs inside opencode itself, so Chat cannot use it.";
  }
  const reason = unusableReason(model);
  return reason === null ? null : `This model ${reason}.`;
}

/** Set or clear `hidden` on a provider's models. An absent entry means
 *  offered, so showing deletes the entry rather than writing `false`. */
function withHidden(
  c: OpencodeCatalogue,
  providerId: string,
  modelIds: string[],
  hidden: boolean,
): OpencodeCatalogue {
  const entry = c.providers[providerId] ?? { models: {} };
  const models = { ...entry.models };
  for (const id of modelIds) {
    if (hidden) models[id] = { hidden: true };
    else delete models[id];
  }
  return { ...c, providers: { ...c.providers, [providerId]: { ...entry, models } } };
}
