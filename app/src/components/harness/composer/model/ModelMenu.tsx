import { useEffect, useMemo, useState, type ComponentProps } from "react";
import { MagnifyingGlass, Star } from "@phosphor-icons/react";
import { PopoverContent } from "@/components/ui/popover";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { ProviderMark } from "@/components/icons/ProviderMark";
import {
  providerLabel,
  reasoningLabel,
  SWITCH_LOCKED,
  type HarnessModel,
  type PickerProvider,
  type Provider,
} from "@/lib/harness";
import { isStarred, useModelStars, type ModelStar } from "@/stores/chat/modelStars";
import { navigateActive } from "@/lib/shell/tabRouters";
import { cn } from "@/lib/utils";
import { modelSummary } from "./modelSummary";
import { EmptyNote, ModelRow, NotInstalled, SectionLabel, StarredList, SwitchLocked } from "./ModelRows";

/** Above this many models the menu grows a search box. */
const SEARCH_THRESHOLD = 8;

export interface ModelMenuProps {
  providers: PickerProvider[];
  provider: Provider;
  /** The agents this chat cannot move to (`switchLocked`); a stable function. */
  locked?: (provider: Provider) => boolean;
  /** Rust's reason for refusing the last pick, shown above the models. */
  refused?: string | null;
  model: string | null;
  reasoning: string | null;
  onProvider: (p: Provider) => void;
  onModel: (m: string | null) => void;
  onReasoning: (level: string | null) => void;
  /** Closes the popover the menu sits in (before a jump to Settings). */
  onClose: () => void;
}

/** A model menu's frame: a column no taller than the room above or below
 *  its trigger, whose model list scrolls between the tabs and the dial. */
export const MODEL_MENU_FRAME =
  "flex max-h-[min(var(--radix-popover-content-available-height),28rem)] w-72 flex-col overflow-hidden";

/** Focus a model menu's search box, which a long catalogue has; false when
 *  there is none. */
export function focusModelSearch(root: HTMLElement | null): boolean {
  const search = root?.querySelector<HTMLElement>("[data-model-search]");
  search?.focus();
  return !!search;
}

/**
 * The model picker's popover over `ModelMenu`, opening on the search box
 * when there is one.
 */
export function ModelMenuContent({
  side,
  align = "start",
  sideOffset = 6,
  collisionPadding,
  ...menu
}: ModelMenuProps & Pick<ComponentProps<typeof PopoverContent>, "side" | "align" | "sideOffset" | "collisionPadding">) {
  return (
    <PopoverContent
      side={side}
      align={align}
      sideOffset={sideOffset}
      collisionPadding={collisionPadding}
      className={cn(MODEL_MENU_FRAME, "p-0")}
      onOpenAutoFocus={(e) => {
        // A long catalogue opens on its search box; otherwise Radix focuses the first tab.
        if (focusModelSearch(e.currentTarget as HTMLElement | null)) e.preventDefault();
      }}
    >
      <ModelMenu {...menu} />
    </PopoverContent>
  );
}

/**
 * The model-and-reasoning menu, drawn inside a `MODEL_MENU_FRAME` column (the
 * picker's popover, or the compact composer's settings page): provider tabs,
 * the active provider's models, and reasoning levels. Nothing inside closes
 * it — agent, model and level are one decision. Neither row has a "default"
 * entry: a turn always names an explicit model and level. Levels come off the
 * model, not the provider. A provider whose CLI is missing shows
 * `NotInstalled` instead of its catalogue, and a `locked` one (a started chat
 * it cannot take over) is dimmed but opens a note saying so, without being
 * picked. Each model row has a star; the Starred tab, first in the strip,
 * lists starred models across agents and opens by default while the chat's
 * model is one of them. It mounts with the frame, so search, peek and tab
 * start afresh at every opening.
 */
export function ModelMenu({
  providers,
  provider,
  locked,
  refused,
  model,
  reasoning,
  onProvider,
  onModel,
  onReasoning,
  onClose,
}: ModelMenuProps) {
  const [query, setQuery] = useState("");
  /** A locked tab the student opened: its note shows, nothing is picked. */
  const [peek, setPeek] = useState<Provider | null>(null);
  const stars = useModelStars((s) => s.stars);
  const toggleStar = useModelStars((s) => s.toggle);
  /** The Starred tab is showing instead of an agent's catalogue; on opening,
   *  only while the chat's model is starred. */
  const [starView, setStarView] = useState(
    () => model != null && isStarred(useModelStars.getState().stars, provider, model),
  );
  /** A starred model on another agent, picked once the switch to it lands. */
  const [pending, setPending] = useState<{ provider: Provider; model: string } | null>(null);

  const { active, availability, unsupported, missing, models, offered, level } = modelSummary({
    providers,
    provider,
    model,
    reasoning,
  });

  const choose = (m: HarnessModel) => {
    onModel(m.id);
    // The CLI rejects a level the new model doesn't take.
    if (!reasoning || !m.reasoningEfforts.includes(reasoning)) {
      onReasoning(m.defaultReasoningEffort ?? m.reasoningEfforts[0] ?? null);
    }
  };

  // A switch may land a render later (or after Rust answers); its catalogue
  // may still be loading. A star the catalogue no longer has opens its tab.
  useEffect(() => {
    if (!pending || pending.provider !== provider) return;
    const p = providers.find((x) => x.id === provider);
    if (!p || p.loading) return;
    const m = p.models.find((x) => x.id === pending.model);
    setPending(null);
    if (m) choose(m);
    else setStarView(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pending, provider, providers]);

  const pickStar = (star: ModelStar) => {
    if (star.provider === provider) {
      const m = models.find((x) => x.id === star.model);
      if (m) choose(m);
      else setStarView(false);
      return;
    }
    if (locked?.(star.provider)) {
      setStarView(false);
      setPeek(star.provider);
      return;
    }
    setPeek(null);
    setPending({ provider: star.provider, model: star.model });
    onProvider(star.provider);
  };

  const showSearch = !starView && !peek && !unsupported && !missing && models.length > SEARCH_THRESHOLD;
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return models;
    return models.filter(
      (m) => m.label.toLowerCase().includes(q) || m.id.toLowerCase().includes(q),
    );
  }, [models, query]);

  // A locked agent's note hides the dial; the trigger still names the level.
  const levels = peek && !starView ? [] : offered;

  return (
    <>
      {providers.length > 1 || stars.length > 0 ? (
        <div className="flex shrink-0 items-center gap-0.5 border-b border-border bg-surface px-2.5 pt-1">
          <button
            type="button"
            title="Starred"
            aria-label="Starred models"
            aria-pressed={starView}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => {
              setPeek(null);
              setStarView(true);
            }}
            className={cn(
              "flex size-8 items-center justify-center border-b-2 transition-colors focus-visible:outline-none",
              starView
                ? "border-foreground text-foreground"
                : "border-transparent text-muted-foreground hover:text-foreground",
            )}
          >
            <Star className="size-4" weight={starView ? "fill" : "regular"} />
          </button>
          {providers.map((p) => {
            const isActive = !starView && p.id === (peek ?? provider);
            // A missing or locked agent is dimmed but clickable: its panel says why.
            const gone = p.health === "missing";
            const shut = p.id !== provider && !!locked?.(p.id);
            return (
              <button
                key={p.id}
                type="button"
                title={shut ? `${p.label} — ${SWITCH_LOCKED}` : gone ? `${p.label} — not installed` : p.label}
                aria-disabled={shut || undefined}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => {
                  setStarView(false);
                  if (shut) setPeek(p.id);
                  else {
                    setPeek(null);
                    if (p.id !== provider) onProvider(p.id);
                  }
                }}
                className={cn(
                  "flex size-8 items-center justify-center border-b-2 transition-colors focus-visible:outline-none",
                  isActive
                    ? "border-foreground text-foreground"
                    : "border-transparent text-muted-foreground hover:text-foreground",
                  (gone || shut) && !isActive && "opacity-40",
                )}
              >
                <ProviderMark provider={p.id} className="size-4" />
              </button>
            );
          })}
        </div>
      ) : null}

      {showSearch ? (
        <div className="flex shrink-0 items-center gap-2 border-b border-border-subtle px-3 py-2">
          <MagnifyingGlass className="size-3.5 shrink-0 text-muted-foreground" />
          <input
            data-model-search
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search models"
            aria-label="Search models"
            className="min-w-0 flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground"
          />
        </div>
      ) : null}

      {(starView || !peek) && refused ? (
        <div
          role="alert"
          className="shrink-0 border-b border-border-subtle px-3 py-2 text-[11px] leading-relaxed text-destructive"
        >
          {refused}
        </div>
      ) : null}

      <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-1 pb-1">
        {starView ? (
          <StarredList
            stars={stars}
            providers={providers}
            provider={provider}
            model={model}
            pending={pending}
            locked={locked}
            onPick={pickStar}
            onUnstar={toggleStar}
          />
        ) : peek ? (
          <SwitchLocked label={providers.find((p) => p.id === peek)?.label ?? providerLabel(peek)} />
        ) : unsupported ? (
          <div className="px-2 py-3 text-xs text-muted-foreground">
            {providerLabel(provider)} — {availability.note}
          </div>
        ) : missing ? (
          <NotInstalled
            label={active?.label ?? ""}
            onSettings={() => {
              onClose();
              navigateActive("/settings/agents");
            }}
          />
        ) : (
          <>
            <SectionLabel>Model</SectionLabel>

            {active?.loading ? (
              <div className="px-2 py-1.5 text-xs text-muted-foreground">Loading models…</div>
            ) : filtered.length === 0 ? (
              query ? (
                <div className="px-2 py-1.5 text-xs text-muted-foreground">
                  No models match your search
                </div>
              ) : active?.emptyNote ? (
                // Empty for a reason fixable in Settings — for opencode, its
                // catalogue filters (`app/src/lib/harness/opencodeCatalogue.ts`).
                <EmptyNote
                  note={active.emptyNote}
                  page={active.emptyNotePage ?? "agents"}
                  onSettings={(page) => {
                    onClose();
                    navigateActive(`/settings/${page}`);
                  }}
                />
              ) : (
                <div className="px-2 py-1.5 text-xs text-muted-foreground">
                  No models available
                </div>
              )
            ) : (
              filtered.map((m) => (
                <ModelRow
                  key={m.id}
                  label={m.label}
                  description={m.description}
                  title={m.id}
                  selected={m.id === model}
                  starred={isStarred(stars, provider, m.id)}
                  onStar={() => toggleStar({ provider, model: m.id, label: m.label })}
                  onClick={() => choose(m)}
                />
              ))
            )}
          </>
        )}
      </div>

      {levels.length > 0 ? (
        <>
          <div className="shrink-0 border-t border-border" />
          <div className="shrink-0 px-2 pb-2.5 pt-2.5">
            <SectionLabel className="mb-2 px-1 py-0">Reasoning</SectionLabel>
            {/* A sunken tray; the chosen level is the raised pill in it. */}
            <ToggleGroup
              type="single"
              spacing={1}
              aria-label="Reasoning"
              value={level ?? ""}
              onValueChange={(v) => {
                // Radix clears on re-click; a level stays chosen.
                if (!v) return;
                onReasoning(v);
              }}
              className="flex w-full rounded-lg bg-muted p-0.5"
            >
              {levels.map((l) => {
                const text = reasoningLabel(l);
                return (
                  <ToggleGroupItem
                    key={l}
                    value={l}
                    aria-label={text}
                    className="h-6 min-w-0 flex-auto shrink-0 whitespace-nowrap rounded-md px-1 text-[11px] font-normal text-muted-foreground shadow-none transition-colors hover:text-foreground data-[state=on]:bg-foreground/12 data-[state=on]:font-semibold data-[state=on]:text-foreground data-[state=on]:shadow-sm"
                  >
                    {text}
                  </ToggleGroupItem>
                );
              })}
            </ToggleGroup>
          </div>
        </>
      ) : null}
    </>
  );
}
