import { useCallback, useEffect, useMemo, useState } from "react";
import { Globe, MagnifyingGlass, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Section } from "@/pages/settings/section";
import { cn } from "@/lib/utils";
import { openExternal } from "@/lib/browser";
import { fmtDayHeading, sqliteUtcToMs } from "@/lib/format";
import {
  clearHistory,
  forgetUrl,
  groupByDay,
  listHistory,
  type HistoryEntry,
} from "@/lib/browserHistory";
import { faviconForHost } from "@/hooks/useBrowserTabs";
import { useBrowserStore } from "@/stores/browserStore";

/**
 * Where the in-app browser has been, grouped by the day it was last there.
 *
 * It lives in Settings rather than as a page of its own because that is what
 * it is for: this is where you come to *forget* something, not to navigate.
 * Anything you actually want to reach again is a keystroke away in the address
 * bar, which ranks the same table (`app/src/lib/browserHistory.ts`).
 *
 * One row per URL, so a page you open every morning appears once, under today
 * — the table has no per-visit log to draw a fuller timeline from, and adding
 * one would mean keeping a record nothing else asks for.
 */
export function BrowserHistorySection() {
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);
  const [query, setQuery] = useState("");
  const [confirming, setConfirming] = useState(false);
  const [clearing, setClearing] = useState(false);
  const favicons = useBrowserStore((s) => s.favicons);

  const reload = useCallback(
    (text: string) =>
      listHistory(text)
        .then(setEntries)
        .catch((cause) => {
          console.error("browser history failed", cause);
          setEntries([]);
        }),
    [],
  );

  useEffect(() => {
    void reload(query);
  }, [reload, query]);

  const days = useMemo(
    () => groupByDay(entries ?? [], sqliteUtcToMs),
    [entries],
  );

  const forget = async (url: string) => {
    await forgetUrl(url).catch(() => {});
    setEntries((rows) => rows?.filter((r) => r.url !== url) ?? rows);
  };

  const wipe = async () => {
    setClearing(true);
    try {
      await clearHistory();
      setEntries([]);
    } catch (cause) {
      console.error("clearing browser history failed", cause);
    } finally {
      setClearing(false);
      setConfirming(false);
    }
  };

  const empty = entries !== null && entries.length === 0;

  return (
    <Section
      title="History"
      description="Pages opened in Oculus's own browser tabs. The address bar suggests from this list."
    >
      <div className="flex items-center gap-2">
        <div className="flex h-8 flex-1 items-center gap-2 rounded-lg border border-border bg-card px-2.5 focus-within:border-brand/50">
          <MagnifyingGlass size={13} className="shrink-0 text-muted-foreground" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search history"
            spellCheck={false}
            className="min-w-0 flex-1 bg-transparent text-[13px] text-foreground outline-none placeholder:text-muted-foreground"
          />
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => setConfirming(true)}
          disabled={entries === null || entries.length === 0}
        >
          Clear history
        </Button>
      </div>

      {/* A list this long scrolls inside its own box rather than growing the
          settings page — the sections under it stay reachable. */}
      <div className="mt-3 max-h-[420px] overflow-y-auto">
        {entries === null ? (
          <p className="py-2 text-xs text-muted-foreground">Loading…</p>
        ) : empty ? (
          <p className="py-2 text-xs text-muted-foreground">
            {query
              ? "Nothing here matches."
              : "Nothing yet — pages you open in a browser tab will show up here."}
          </p>
        ) : (
          days.map(({ day, entries: rows }) => (
            <div key={day} className="mb-4 last:mb-0">
              <h3 className="sticky top-0 z-10 bg-card/95 py-1 text-[11px] font-medium text-muted-foreground backdrop-blur-sm">
                {fmtDayHeading(day)}
              </h3>
              {rows.map((entry) => {
                const icon = faviconForHost(entry.host, favicons);
                return (
                  <div
                    key={entry.url}
                    className={cn(
                      "group flex items-center gap-2.5 rounded-md px-2 py-1.5",
                      "hover:bg-sidebar-item-hover",
                    )}
                  >
                    <span className="flex h-3.5 w-3.5 shrink-0 items-center justify-center text-muted-foreground">
                      {icon ? (
                        <img
                          src={icon}
                          alt=""
                          className="h-3.5 w-3.5 rounded-[2px] object-contain"
                        />
                      ) : (
                        <Globe size={13} />
                      )}
                    </span>
                    <button
                      type="button"
                      onClick={() => void openExternal(entry.url)}
                      className="min-w-0 flex-1 text-left"
                    >
                      <span className="block truncate text-[12px] text-foreground">
                        {entry.title || entry.url}
                      </span>
                      <span className="block truncate text-[11px] text-muted-foreground">
                        {entry.url}
                      </span>
                    </button>
                    {entry.visits > 1 && (
                      <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">
                        {entry.visits}×
                      </span>
                    )}
                    <button
                      type="button"
                      onClick={() => void forget(entry.url)}
                      aria-label={`Forget ${entry.url}`}
                      className="shrink-0 rounded-md p-1 text-muted-foreground opacity-0 transition-opacity hover:bg-sidebar-item-active hover:text-foreground group-hover:opacity-100"
                    >
                      <X size={11} />
                    </button>
                  </div>
                );
              })}
            </div>
          ))
        )}
      </div>

      <Dialog open={confirming} onOpenChange={(open) => !open && !clearing && setConfirming(false)}>
        <DialogContent className="sm:max-w-md" showCloseButton={false}>
          <DialogHeader>
            <DialogTitle>Clear browsing history?</DialogTitle>
            <DialogDescription asChild>
              <div className="space-y-2.5">
                <p>
                  All{" "}
                  <span className="tabular-nums">
                    {(entries?.length ?? 0).toLocaleString()}
                  </span>{" "}
                  pages are forgotten, and the address bar stops suggesting
                  them. This cannot be undone.
                </p>
                <p>
                  Open tabs are not touched, and neither are the sites you are
                  signed in to — this is the list of where you have been, not
                  the cookies that got you there.
                </p>
              </div>
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setConfirming(false)} disabled={clearing}>
              Cancel
            </Button>
            <Button onClick={() => void wipe()} disabled={clearing}>
              {clearing ? "Clearing…" : "Clear history"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Section>
  );
}
