import { useEffect, useMemo, useState } from "react";
import { CircleNotch, MagnifyingGlass, Plus } from "@phosphor-icons/react";

import {
  cachedProviders,
  forgetProviders,
  opencodeDisconnect,
  opencodeProviders,
  rememberProviders,
  type OpencodeProvider,
  type OpencodeProviderList,
} from "@/lib/opencodeAuth";
import { providerHealth, useBridgeHealth } from "@/hooks/useBridgeHealth";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Section } from "@/pages/settings/section";
import { OpencodeConnectDialog } from "./OpencodeConnectDialog";

/** How many search results a query shows before it is asking the wrong
 *  question. Long enough to include the one you meant, short enough that the
 *  section never becomes a scrolling catalogue. */
const RESULTS = 8;

/**
 * Which opencode providers this machine can reach, and how to change that.
 *
 * Chat drives three CLIs. Two of them are signed in as the student and carry
 * their subscription; opencode carries whatever `opencode auth` holds, and
 * before this the only way to put something there was to open a terminal — so
 * the model picker showed two providers out of two hundred and eighteen and
 * nothing on screen said why. Every action here goes through opencode's own
 * server (`app/src/lib/opencodeAuth.ts`), so a credential lands in opencode's
 * store and is the same one the student's terminal opencode uses.
 *
 * Three decisions shape what is drawn.
 *
 * **Opening Settings must not start a CLI.** Reading the list means asking the
 * app's `opencode serve`, and starting it on a page visit is the rule
 * `harness_refresh_rate_limits` already declines to break. So the section opens
 * on a button and the student's click is what spawns it; after that the answer
 * is kept for the window's life, so coming back costs nothing. Whether opencode
 * is installed at all is a separate, free question — `useBridgeHealth` has
 * already answered it — which is why a machine without opencode says so instead
 * of failing a call.
 *
 * **A wall of 218 rows is not a list.** What is connected leads, because that
 * is the thing the picker will show and the thing a student might want gone.
 * Under it, the handful of providers that declare a real sign-in flow, which is
 * what "connect an account" means to most people. The other two hundred are
 * reachable by name through the search box, not by scrolling.
 *
 * **A provider from `opencode.json` is not a sign-in.** `config` providers are
 * connected because they are declared, so there is no credential to remove and
 * the row says where it came from rather than offering a button that would do
 * nothing.
 */
export function OpencodeProvidersSection() {
  const { health } = useBridgeHealth();
  const installed = providerHealth(health, "opencode");

  const [list, setList] = useState<OpencodeProviderList | null>(cachedProviders);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [connecting, setConnecting] = useState<OpencodeProvider | null>(null);
  const [removing, setRemoving] = useState<string | null>(null);

  // A cached list from an earlier visit is only trustworthy while opencode is
  // the same install; a recheck that finds it gone should not leave 218 rows
  // on screen offering to sign in.
  useEffect(() => {
    if (installed === "missing") {
      forgetProviders();
      setList(null);
    }
  }, [installed]);

  const load = async (refresh: boolean) => {
    setLoading(true);
    setError(null);
    try {
      setList(rememberProviders(await opencodeProviders(refresh)));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const disconnect = async (id: string) => {
    setRemoving(id);
    setError(null);
    try {
      setList(rememberProviders(await opencodeDisconnect(id)));
    } catch (e) {
      setError(String(e));
    } finally {
      setRemoving(null);
    }
  };

  const providers = list?.providers ?? [];
  const connected = useMemo(() => providers.filter((p) => p.connected), [providers]);
  /** The ten or so that declare a named way in — an OAuth flow, or a key plus
   *  the fields that key needs. Everything else is an unadorned API key and is
   *  found by name. */
  const featured = useMemo(
    () => providers.filter((p) => !p.connected && p.methods.some((m) => m.kind === "oauth")),
    [providers],
  );
  const results = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return [];
    return providers
      .filter((p) => p.name.toLowerCase().includes(q) || p.id.toLowerCase().includes(q))
      .slice(0, RESULTS);
  }, [providers, query]);

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
      {list === null ? (
        <div className="flex items-center gap-3 py-1">
          <Button variant="outline" size="sm" disabled={loading} onClick={() => void load(false)}>
            {loading ? <CircleNotch size={12} className="animate-spin" /> : <Plus size={12} />}
            Manage providers
          </Button>
          <span className="text-xs text-muted-foreground">
            Reading the list starts opencode in the background.
          </span>
        </div>
      ) : (
        <div className="flex flex-col gap-4">
          <div>
            <Header>Connected</Header>
            <div className="divide-y divide-border-subtle">
              {connected.map((p) => (
                <Row key={p.id} provider={p}>
                  {p.source === "config" ? (
                    <span className="text-xs text-muted-foreground">from opencode.json</span>
                  ) : (
                    <Button
                      variant="ghost"
                      size="xs"
                      disabled={removing === p.id}
                      onClick={() => void disconnect(p.id)}
                    >
                      {removing === p.id && <CircleNotch size={12} className="animate-spin" />}
                      Disconnect
                    </Button>
                  )}
                </Row>
              ))}
              {connected.length === 0 && (
                <p className="py-2.5 text-xs text-muted-foreground">
                  Nothing is signed in yet, so Chat's opencode agent has no models to run.
                </p>
              )}
            </div>
          </div>

          <div>
            <Header>Add a provider</Header>
            <div className="relative">
              <MagnifyingGlass
                size={13}
                className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-muted-foreground"
              />
              <Input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder={`Search ${providers.length} providers`}
                className="pl-8"
              />
            </div>
            <div className="mt-1 divide-y divide-border-subtle">
              {(query.trim() ? results : featured).map((p) => (
                <Row key={p.id} provider={p}>
                  <Button variant="ghost" size="xs" onClick={() => setConnecting(p)}>
                    Connect
                  </Button>
                </Row>
              ))}
              {query.trim() && results.length === 0 && (
                <p className="py-2.5 text-xs text-muted-foreground">
                  No provider called “{query.trim()}”.
                </p>
              )}
              {!query.trim() && (
                <p className="py-2.5 text-xs text-muted-foreground">
                  {featured.length > 0
                    ? "These offer a sign-in flow. Every other provider takes an API key — search for it by name."
                    : "Search for a provider by name to add an API key."}
                </p>
              )}
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

      {error && <p className="mt-2 text-xs text-destructive">{error}</p>}

      {connecting && (
        <OpencodeConnectDialog
          provider={connecting}
          onClose={() => setConnecting(null)}
          onDone={(next) => {
            setList(rememberProviders(next));
            setConnecting(null);
            setQuery("");
          }}
        />
      )}
    </Section>
  );
}

function Header({ children }: { children: React.ReactNode }) {
  return <div className="mb-1 text-xs font-medium text-muted-foreground">{children}</div>;
}

function Row({
  provider,
  children,
}: {
  provider: OpencodeProvider;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-4 py-2">
      <div className="min-w-0">
        <div className="truncate text-[13px] text-foreground">{provider.name}</div>
        <div className="mt-0.5 truncate text-xs text-muted-foreground">
          <span className="tabular-nums">{provider.modelCount}</span>{" "}
          {provider.modelCount === 1 ? "model" : "models"}
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}
