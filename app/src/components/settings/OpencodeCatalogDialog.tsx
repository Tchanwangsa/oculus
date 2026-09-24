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

/** How many search results a query shows before it is asking the wrong
 *  question. It was eight when the results shared a column with the connected
 *  list and had two rows of room; now that a query gives the whole body over
 *  to its matches it can be a list worth scrolling, and the cap is only there
 *  so that a one-letter query is not all 218 rows. */
const RESULTS = 30;

type View = { kind: "providers" } | { kind: "models"; providerId: string };

/**
 * Everything about opencode's providers, in one dialog: what is connected,
 * what each provider's models have been measured to do, and which of them the
 * composer is allowed to offer.
 *
 * It replaced a version of all this inlined into Settings → AI, which was the
 * wrong shape twice over. A 218-row search box and a connected list are not a
 * settings *row*; and the thing that actually had to be built — per-model
 * ticks over a provider that can contribute three hundred of them — has no
 * chance of fitting on a page beside four other sections.
 *
 * **Three views, one dialog.** `providers` → `models:<id>` → back, held in
 * local state rather than as three `<Dialog>`s, because they are one task: a
 * student who connects OpenRouter is going to check it and then untick two
 * hundred rows, and a stack of dialogs would make each step feel like leaving
 * the last one. The connect flow is the exception and genuinely is a second
 * dialog — it is a form with its own browser round trip
 * (`OpencodeConnectDialog.tsx`), and it is rendered over this one unchanged.
 *
 * **The dialog's first read is what starts opencode.** Listing providers means
 * asking the app's `opencode serve`, and a settings page being *opened* is not
 * a reason to spawn a CLI — so the section above draws a button, and mounting
 * this component is the click. After that the answer is cached for the
 * window's life in `opencodeAuth.ts`.
 *
 * **Connected leads, because it is the only thing there is to remove**, and
 * removal has two shapes that are not the same fact. A provider with a
 * credential is disconnected. A provider whose `source` is `config` is
 * connected because the student's own `opencode.json` declares it: there is no
 * credential to delete, and Oculus must never edit that file — so the honest
 * action is to hide it from Oculus, which is a fact about this app and nothing
 * else. That is `hiddenProviders` in `app/src/lib/opencodeCatalogue.ts`.
 *
 * **What the model list is, and what it is not.** The rows come from
 * `harnessOpencodeModels()` — opencode's whole catalogue — and never from
 * `PROVIDERS`' opencode `fetchModels`, which is already filtered by the very
 * store this dialog edits. Reading through that filter would mean a model
 * could only be unhidden if it were not hidden.
 */
export function OpencodeCatalogDialog({ onClose }: { onClose: () => void }) {
  const [view, setView] = useState<View>({ kind: "providers" });
  const [list, setList] = useState<OpencodeProviderList | null>(cachedProviders);
  // True on the first render already when there is nothing cached, so the
  // view never flashes its failure state in the beat before the effect runs.
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

  // Mounting is the click that was allowed to start opencode. A list this
  // window already read is reused rather than re-asked — nothing about the
  // credential store changes while nobody is editing it.
  useEffect(() => {
    if (!cachedProviders()) void loadList(false);
  }, [loadList]);

  /**
   * opencode's whole catalogue, held for the life of the dialog.
   *
   * Every model of every provider arrives in one call, so the second provider
   * a student opens costs nothing. The ref beside the state is what a click
   * handler reads: `Check` has to hand the sweep a list of ids *now*, and a
   * `useState` value captured in a closure would be the one from the render
   * that drew the button.
   */
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

  // Read on open rather than on the first click into a provider, because the
  // self-check below needs model ids to ask for. It is one `GET /provider`
  // against the server this dialog has already started.
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
      // The credential is gone, so the 401s it produced are no longer facts
      // about anything. Reconnecting later starts from an empty catalogue
      // rather than inheriting a menu an old key emptied.
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

  // A hidden provider is listed once, under *Hidden* — it is still connected
  // as far as opencode is concerned, and drawing it in both groups would make
  // "hide" look like it had not worked.
  const connected = useMemo(
    () => providers.filter((p) => p.connected && !hiddenIds.includes(p.id)),
    [providers, hiddenIds],
  );
  const hidden = useMemo(
    () => providers.filter((p) => hiddenIds.includes(p.id)),
    [providers, hiddenIds],
  );
  /** The ten or so that declare a named way in — an OAuth flow, or a key plus
   *  the fields that key needs. Everything else is an unadorned API key and is
   *  found by name. */
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

      {/* Over the top of this one, and unchanged: the connect flow is a form
          with its own browser round trip, and it is the one step here that is
          genuinely a separate task. */}
      {connecting && (
        <OpencodeConnectDialog
          provider={connecting}
          onClose={() => setConnecting(null)}
          onDone={(next) => {
            const id = connecting.id;
            setList(rememberProviders(next));
            setConnecting(null);
            setQuery("");
            // The model catalogue this dialog is holding predates the
            // provider, so its models are not in it. Re-read, and land on its
            // model list: everything opencode lists is offered already, so
            // there is nothing to wait for and nothing to spend.
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
 * What is connected, what is hidden, and the way in to the other 213 — in one
 * column with **one scroll region**, under a search field that is the only
 * pinned thing.
 *
 * The obvious layout is the wrong one and was built first: connected list,
 * hidden list, then a search with its results under it, all sharing a single
 * scroll. Every group competes with every other for the same column, and the
 * one that loses is always the results — a student with five providers signed
 * in was left about two rows of room to scroll two hundred in, and pinning the
 * top half only moved the squeeze.
 *
 * **The query is the mode.** Empty, the body is the student's own providers,
 * and the whole dialog is theirs to scroll. Typing gives the body over to the
 * matches, because looking for one of 218 providers is a find rather than a
 * browse, and nothing about the connected list helps with it. So the two
 * things never share a column at all, the search stays reachable from anywhere
 * in either, and neither can starve the other of height.
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
  /** Every model opencode lists, or null before the read lands. Only the
   *  counts need it, which is why a row draws without waiting for it. */
  models: HarnessModel[] | null;
  removing: string | null;
  onModels: (providerId: string) => void;
  onDisconnect: (p: OpencodeProvider) => void;
  onHide: (providerId: string, hidden: boolean) => void;
  onConnect: (p: OpencodeProvider) => void;
}) {
  if (list === null) {
    // A read that failed leaves nothing to draw, and a spinner over it would
    // claim work that is not happening — the message below this view says what
    // went wrong, so this offers the one thing that can change it.
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

/**
 * What a provider row lets you do, in one place so that a search result and a
 * row under *Connected* cannot offer different things about the same provider
 * — typing "zen" used to produce a *Connect* button for a provider that was
 * already signed in.
 *
 * **Disconnect is an icon.** Three words of chrome on every row — *Models*,
 * *Check*, *Disconnect* — made the provider's own name the least emphatic
 * thing in it. *Models* keeps its label because it is the way further in and
 * the one a student is looking for; the other is the kind of thing an icon and
 * a tooltip carry. *Check* is gone altogether: it sent a billed request to
 * every model the provider has.
 */
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
        // Declared in the student's own opencode.json, so there is no
        // credential to delete and Oculus will not touch that file. Hiding is
        // the only removal that is true.
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

/** A ghost icon button whose tooltip is its accessible name, so the two cannot
 *  drift apart. */
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
  /** Greys the name. Set under *Hidden*, where the row is a record of
   *  something switched off rather than something in use. */
  muted?: boolean;
  /** Set for a row in the *Hidden* group, where a count of offered models
   *  would be a number the picker ignores. */
  hiddenNote?: string;
  children: React.ReactNode;
}) {
  // How many of this provider's models the composer will show: all of them,
  // less the ones that cannot run here and the ones unticked. A provider in
  // the add list is not signed in, so that number is a question about nothing
  // — its size is what a student is weighing there. Before the model list
  // lands there is nothing to subtract, so the row says the size and catches
  // up rather than drawing a count it would have to correct.
  const mine = models?.filter((m) => providerOf(m.id) === provider.id) ?? null;
  // `filterOffered` rather than a local test: it is the one place the
  // capability half and the catalogue half of the gate are applied together,
  // so this line and the composer's menu cannot come to different numbers.
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

  // *Selected* narrows the list to what is ticked — but to what was ticked
  // when it was chosen, not live. A live filter would pull a row out from
  // under the click that unticked it, and a mis-click would leave nothing to
  // click back on. Choosing *Selected* again re-reads the ticks.
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

  // A blocked row — Zen, or missing a capability the agent needs — stays in
  // the list so the student can read why, but it is not offered and not
  // tickable. Counting it as offered would be the same wrong promise the
  // disabled checkbox refuses to make.
  const blocked = models.filter((m) => blockedBecause(m) !== null).length;
  const usable = models.filter((m) => blockedBecause(m) === null);
  const hidden = usable.filter((m) => entry?.models[m.id]?.hidden === true).length;
  const offered = Math.max(models.length - blocked - hidden, 0);
  const status =
    blocked > 0
      ? `${offered} of ${models.length} offered · ${blocked} cannot run here`
      : `${offered} of ${models.length} offered`;

  // Bulk ticks act on what the search has narrowed to, and only on rows there
  // is something to change about.
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

  // **A model that cannot work here cannot be ticked.** The tick is a promise
  // that the composer will offer this model, and the gate will not offer one
  // whose gateway always refuses (Zen) or that cannot call a tool — so an
  // enabled checkbox here would be a control that appears to work and changes
  // nothing. Both are known without asking anybody, which is the whole bar a
  // rule has to clear to live here; everything else shows up in the timeline.
  const why = blockedBecause(model);
  const blocked = why !== null;

  // The **row** is the control, not the box inside it. A provider like
  // OpenRouter is three hundred of these, and a 16px target beside a 13px name
  // is the wrong thing to have to hit three hundred times. A `<label>` around
  // it would be the usual trick and is not safe here: shadcn's Checkbox is a
  // Radix `button` rather than an `input`, so it reaches the control only
  // through label activation, and a row that toggles twice is worse than one
  // that toggles from further away.
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
        {/* An id is an id: shown verbatim, never humanized, and never in a
            monospace face — this app keeps that for code. */}
        <div className="mt-0.5 truncate text-xs text-muted-foreground">{model.id}</div>
        {why && <div className="mt-0.5 text-xs text-muted-foreground">{why}</div>}
      </div>
    </div>
  );
}

/**
 * A list with nothing in it, and the way out of it.
 *
 * An empty list here is most of the dialog's height, and the only exit from a
 * provider that turned out to have no models was a 24px caret in the corner of
 * the header — a long way from where the eye is, which is the middle of the
 * empty space. So the action that ends the dead end goes there: *back* when
 * there was never anything to show, *clear* when a query is what emptied it.
 */
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

/** Why this row cannot reach the composer, or null. The two free refusals in
 *  one sentence each, so the list and its counts cannot disagree about which
 *  rows are on offer. Zen leads because it is the stronger fact: those models
 *  claim every capability and still refuse everything. */
function blockedBecause(model: HarnessModel): string | null {
  if (isZen(model.id)) {
    return "opencode’s free tier only runs inside opencode itself, so Chat cannot use it.";
  }
  const reason = unusableReason(model);
  return reason === null ? null : `This model ${reason}.`;
}

/** How many of a provider's models the student has unticked. The catalogue
 *  stores nothing else now, so *offered* is the provider's model count less
 *  this — computed by the caller, which is the only place that knows how many
 *  models opencode lists. Exported for the section's summary line. */
export function countHidden(c: OpencodeCatalogue | null, providerId: string): number {
  const models = c?.providers[providerId]?.models ?? {};
  return Object.values(models).filter((m) => m.hidden === true).length;
}

/** Set or clear `hidden` on a provider's models.
 *
 * Unlike the version this replaced it **creates** an entry for a model that
 * has none, because an absent entry now means offered: there is nothing else
 * in a row to preserve, and a provider the student has never touched has no
 * entries at all to edit. */
function withHidden(
  c: OpencodeCatalogue,
  providerId: string,
  modelIds: string[],
  hidden: boolean,
): OpencodeCatalogue {
  const entry = c.providers[providerId] ?? { models: {} };
  const models = { ...entry.models };
  for (const id of modelIds) {
    // Showing a model *removes* its row rather than writing `hidden: false`:
    // absent is what `parse` keeps and what `isOffered` reads as offered, so a
    // catalogue never accumulates entries that say nothing.
    if (hidden) models[id] = { hidden: true };
    else delete models[id];
  }
  return { ...c, providers: { ...c.providers, [providerId]: { ...entry, models } } };
}
