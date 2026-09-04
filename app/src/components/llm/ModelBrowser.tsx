import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowsClockwise, Check, CircleNotch, MagnifyingGlass } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { sameModel, type LlmProvider, type ModelRef } from "@/lib/db";
import { cn } from "@/lib/utils";

/** One entry of a provider's catalogue — mirrors `ModelInfo` in
 *  `app/src-tauri/src/llm.rs`. */
export interface ModelInfo {
  id: string;
  name: string | null;
  contextLength: number | null;
  promptPricePerM: number | null;
}

/** Rendering every OpenRouter model at once is thousands of rows for nothing;
 *  the search box is the way through the list, so cap what is drawn. */
const MAX_ROWS = 150;

function matches(m: ModelInfo, query: string) {
  const haystack = `${m.id} ${m.name ?? ""}`.toLowerCase();
  return query
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((term) => haystack.includes(term));
}

function price(m: ModelInfo) {
  if (m.promptPricePerM == null) return null;
  if (m.promptPricePerM === 0) return "free";
  return `$${m.promptPricePerM < 1 ? m.promptPricePerM.toFixed(2) : m.promptPricePerM.toFixed(1)}/M`;
}

/** Browse one provider's catalogue and pick the models worth keeping. Toggling
 *  a row adds or removes it from the library immediately — there is no "save",
 *  because the library is the only thing being edited. */
export function ModelBrowser({
  open,
  onOpenChange,
  providers,
  library,
  onToggle,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  providers: LlmProvider[];
  library: ModelRef[];
  onToggle: (m: ModelRef) => void;
}) {
  const [providerId, setProviderId] = useState(providers[0]?.id ?? "");
  const [query, setQuery] = useState("");
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async (id: string) => {
    if (!id) return;
    setLoading(true);
    setError(null);
    try {
      setModels(await invoke<ModelInfo[]>("llm_list_models", { providerId: id }));
    } catch (e) {
      setModels([]);
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  // Fetch on open and whenever the provider changes; a catalogue is never
  // cached, so opening the dialog is always a fresh list.
  useEffect(() => {
    if (!open) return;
    const id = providers.some((p) => p.id === providerId)
      ? providerId
      : (providers[0]?.id ?? "");
    if (id !== providerId) setProviderId(id);
    load(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, providerId]);

  const filtered = useMemo(
    () => (query.trim() ? models.filter((m) => matches(m, query)) : models),
    [models, query],
  );
  const shown = filtered.slice(0, MAX_ROWS);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Add models</DialogTitle>
          <DialogDescription>
            Everything a provider serves. Pick the ones you want in your library —
            they are what the chat, summaries and fallbacks choose from.
          </DialogDescription>
        </DialogHeader>

        <div className="flex items-center gap-2">
          <Select value={providerId} onValueChange={setProviderId}>
            <SelectTrigger size="sm" className="h-8 w-44 text-xs">
              <SelectValue placeholder="Provider" />
            </SelectTrigger>
            <SelectContent>
              {providers.map((p) => (
                <SelectItem key={p.id} value={p.id} className="text-xs">
                  {p.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>

          <div className="relative flex-1">
            <MagnifyingGlass
              size={13}
              className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground"
            />
            <Input
              autoFocus
              className="h-8 pl-7 text-xs"
              placeholder="Search models…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>

          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="Refresh"
            onClick={() => load(providerId)}
            disabled={loading}
          >
            {loading ? (
              <CircleNotch size={13} className="animate-spin" />
            ) : (
              <ArrowsClockwise size={13} />
            )}
          </Button>
        </div>

        {error && <p className="text-xs text-destructive">{error}</p>}

        <div className="h-96 overflow-y-auto -mx-2 px-2">
          {loading && models.length === 0 ? (
            <p className="text-xs text-muted-foreground py-2">Loading catalogue…</p>
          ) : shown.length === 0 ? (
            <p className="text-xs text-muted-foreground py-2">
              {error ? "Could not reach this provider." : "No models match."}
            </p>
          ) : (
            <div className="flex flex-col">
              {shown.map((m) => {
                const ref = { providerId, model: m.id };
                const inLibrary = library.some((l) => sameModel(l, ref));
                return (
                  <button
                    key={m.id}
                    type="button"
                    onClick={() => onToggle(ref)}
                    className={cn(
                      "flex items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors",
                      inLibrary ? "text-foreground" : "text-muted-foreground",
                      "hover:bg-accent hover:text-foreground",
                    )}
                  >
                    <Check
                      size={13}
                      weight="bold"
                      className={cn("shrink-0", inLibrary ? "text-primary" : "opacity-0")}
                    />
                    <span className="truncate text-xs flex-1">{m.id}</span>
                    {m.contextLength != null && (
                      <span className="text-[11px] text-muted-foreground tabular-nums shrink-0">
                        {Math.round(m.contextLength / 1000)}k
                      </span>
                    )}
                    {price(m) && (
                      <span className="text-[11px] text-muted-foreground tabular-nums shrink-0 w-16 text-right">
                        {price(m)}
                      </span>
                    )}
                  </button>
                );
              })}
            </div>
          )}
        </div>

        <p className="text-xs text-muted-foreground">
          {filtered.length > MAX_ROWS
            ? `Showing ${MAX_ROWS} of ${filtered.length} matches — keep typing to narrow it down.`
            : `${filtered.length} of ${models.length} models`}
        </p>
      </DialogContent>
    </Dialog>
  );
}
