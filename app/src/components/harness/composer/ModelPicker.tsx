import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { CaretDown, Check, LockSimple, MagnifyingGlass, Star } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ProviderMark } from "@/components/icons/ProviderMark";
import {
  providerAvailability,
  providerLabel,
  reasoningLabel,
  SWITCH_LOCKED,
  type HarnessModel,
  type PickerProvider,
  type Provider,
} from "@/lib/harness";
import { isStarred, useModelStars, type ModelStar } from "@/stores/chat/modelStars";
import { settingsPage, type SettingsPageId } from "@/lib/search/settings";
import { navigateActive } from "@/lib/shell/tabRouters";
import { cn } from "@/lib/utils";

/** `locked` for a picker whose chat keeps its agent whatever the bridges can do. */
export const LOCK_OTHERS = () => true;

/** Above this many models the menu grows a search box. */
const SEARCH_THRESHOLD = 8;

export type { PickerProvider } from "@/lib/harness";

/**
 * The composer's model-and-reasoning switcher (after bb's
 * `ModelReasoningPicker`): provider tabs, the active provider's models, and
 * reasoning levels. Nothing inside closes it — agent, model and level are one
 * decision. Neither row has a "default" entry: a turn always names an explicit
 * model and level. Levels come off the model, not the provider. A provider
 * whose CLI is missing shows `NotInstalled` instead of its catalogue, and a
 * `locked` one (a started chat it cannot take over) is dimmed but opens a
 * note saying so, without being picked. Each model row has a star; the
 * Starred tab, first in the strip, lists starred models across agents and
 * opens by default while the chat's model is one of them. Stable selection props keep composer
 * draft keystrokes outside this catalogue's memo boundary.
 */
export const ModelPicker = memo(function ModelPicker({
  providers,
  provider,
  locked,
  refused,
  switchFrom,
  model,
  reasoning,
  onProvider,
  onModel,
  onReasoning,
  align = "start",
  open: openProp,
  onOpenChange,
  emptyLabel = "Choose a model",
  emptyIcon,
  keepEmptyTrigger = false,
  className,
}: {
  providers: PickerProvider[];
  provider: Provider;
  /** The agents this chat cannot move to (`switchLocked`); a stable function. */
  locked?: (provider: Provider) => boolean;
  /** Rust's reason for refusing the last pick, shown above the models. */
  refused?: string | null;
  /** The agent the chat moves away from with its next message, when the
   *  pick names another (`pendingSwitch`). */
  switchFrom?: Provider | null;
  model: string | null;
  reasoning: string | null;
  onProvider: (p: Provider) => void;
  onModel: (m: string | null) => void;
  onReasoning: (level: string | null) => void;
  /** Which trigger edge the menu lines up with; `end` for a trigger at a page's right edge. */
  align?: "start" | "end";
  /** Given, the caller owns whether the menu is open (the composer's `/model`
   *  opens it); `onOpenChange` then hears every change. */
  open?: boolean;
  /** Told whenever the menu opens or closes. */
  onOpenChange?: (open: boolean) => void;
  /** The trigger's text while no model is set. */
  emptyLabel?: string;
  /** Replaces the agent's mark while no model is set, for an "add" trigger;
   *  null shows none. */
  emptyIcon?: ReactNode;
  /** The trigger keeps its empty look with a model set: an "add" menu whose
   *  pick lands in a row of its own. */
  keepEmptyTrigger?: boolean;
  className?: string;
}) {
  const [openState, setOpenState] = useState(false);
  const open = openProp ?? openState;
  const setOpen = (next: boolean) => {
    setOpenState(next);
    onOpenChange?.(next);
  };
  const [query, setQuery] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  /** A locked tab the student opened: its note shows, nothing is picked. */
  const [peek, setPeek] = useState<Provider | null>(null);
  const stars = useModelStars((s) => s.stars);
  const toggleStar = useModelStars((s) => s.toggle);
  /** The Starred tab is showing instead of an agent's catalogue. */
  const [starView, setStarView] = useState(false);
  /** A starred model on another agent, picked once the switch to it lands. */
  const [pending, setPending] = useState<{ provider: Provider; model: string } | null>(null);

  const active = providers.find((p) => p.id === provider);
  const availability = providerAvailability(provider);
  const unsupported = !availability.supported;
  const models = unsupported ? [] : active?.models ?? [];
  const selected = models.find((m) => m.id === model) ?? null;

  // A stale search would silently hide rows the next time the menu opens.
  useEffect(() => {
    if (!open) {
      setQuery("");
      setPeek(null);
      setPending(null);
    }
  }, [open]);

  // Laid out before paint, so the menu never flashes the agent's tab first.
  useLayoutEffect(() => {
    if (open) setStarView(model != null && isStarred(useModelStars.getState().stars, provider, model));
    // Only on opening: the tab the student picks afterwards stays.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

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

  const missing = active?.health === "missing";

  const showSearch = !starView && !peek && !unsupported && !missing && models.length > SEARCH_THRESHOLD;
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return models;
    return models.filter(
      (m) => m.label.toLowerCase().includes(q) || m.id.toLowerCase().includes(q),
    );
  }, [models, query]);

  const offered = unsupported || missing ? [] : selected?.reasoningEfforts ?? [];
  const level = reasoning && offered.includes(reasoning) ? reasoning : null;
  // A locked agent's note hides the dial; the trigger still names the level.
  const levels = peek && !starView ? [] : offered;

  // An id with no row (a model this list doesn't know) shows as the raw id.
  const label = selected?.label ?? model;
  const levelText = level ? reasoningLabel(level) : null;
  const title = [
    active?.label ?? providerLabel(provider),
    selected?.id ?? model,
    levelText && `${levelText} reasoning`,
    missing && "not installed",
    unsupported && availability.note,
    switchFrom && "from your next message",
  ]
    .filter(Boolean)
    .join(" · ");

  const trigger = (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      aria-label="Model and reasoning"
      title={title}
      className={cn(
        "h-6 max-w-[280px] gap-1.5 px-1.5 text-[11px] font-normal text-muted-foreground hover:text-foreground",
        className,
      )}
    >
      {(label == null || keepEmptyTrigger) && emptyIcon !== undefined ? (
        emptyIcon
      ) : (
        <ProviderMark provider={provider} className="size-3.5 shrink-0" />
      )}
      <span className={cn("min-w-0 truncate", label != null && !keepEmptyTrigger && "text-foreground")}>
        {keepEmptyTrigger ? emptyLabel : (label ?? emptyLabel)}
      </span>
      {levelText && !keepEmptyTrigger ? (
        <span className="shrink-0 text-muted-foreground">{levelText}</span>
      ) : null}
      <CaretDown className="size-3 shrink-0 text-muted-foreground" />
    </Button>
  );

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>{trigger}</PopoverTrigger>
      <PopoverContent
        align={align}
        sideOffset={6}
        className="flex max-h-[min(var(--radix-popover-content-available-height),28rem)] w-auto max-w-[calc(100vw-16px)] overflow-hidden p-0"
        onOpenAutoFocus={(e) => {
          if (!showSearch) return;
          e.preventDefault();
          searchRef.current?.focus();
        }}
      >
        <div className="flex w-72 min-w-0 shrink flex-col">
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
              ref={searchRef}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search models"
              aria-label="Search models"
              className="min-w-0 flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground"
            />
          </div>
        ) : null}

        {(starView || !peek) && (refused || switchFrom) ? (
          <div
            role={refused ? "alert" : undefined}
            className={cn(
              "shrink-0 border-b border-border-subtle px-3 py-2 text-[11px] leading-relaxed",
              refused ? "text-destructive" : "text-muted-foreground",
            )}
          >
            {refused ?? `Switches from ${providerLabel(switchFrom!)} with your next message.`}
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
                setOpen(false);
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
                      setOpen(false);
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

        </div>

        {levels.length > 0 ? (
          <ReasoningColumn
            levels={levels}
            level={level}
            defaultLevel={selected?.defaultReasoningEffort ?? null}
            onReasoning={onReasoning}
          />
        ) : null}
      </PopoverContent>
    </Popover>
  );
});

/** The chosen model's levels, weakest first, down the menu's right side:
 *  the chosen one ticked, the model's own default tagged. */
function ReasoningColumn({
  levels,
  level,
  defaultLevel,
  onReasoning,
}: {
  levels: string[];
  level: string | null;
  defaultLevel: string | null;
  onReasoning: (level: string) => void;
}) {
  return (
    <div role="radiogroup" aria-label="Reasoning" className="flex w-44 shrink-0 flex-col overflow-y-auto overscroll-contain border-l border-border px-1 pb-1">
      <SectionLabel>Reasoning</SectionLabel>
      {levels.map((l) => {
        const on = l === level;
        return (
          <button
            key={l}
            type="button"
            role="radio"
            aria-checked={on}
            onClick={() => onReasoning(l)}
            className={cn(
              "flex w-full cursor-default select-none items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs outline-none transition-colors hover:bg-accent focus-visible:bg-accent",
              on ? "text-foreground" : "text-muted-foreground hover:text-foreground",
            )}
          >
            <span className="min-w-0 truncate">{reasoningLabel(l)}</span>
            {l === defaultLevel ? <span className="text-[11px] text-muted-foreground">Default</span> : null}
            <Check className={cn("ml-auto size-3.5 shrink-0", on ? "opacity-100" : "opacity-0")} />
          </button>
        );
      })}
    </div>
  );
}

/** Shown in place of a locked agent's catalogue: this chat cannot move to it. */
function SwitchLocked({ label }: { label: string }) {
  return (
    <div className="flex items-start gap-2 px-2 py-3 text-xs text-muted-foreground">
      <LockSimple className="mt-0.5 size-3.5 shrink-0" aria-hidden />
      <span>
        {label} — {SWITCH_LOCKED}
      </span>
    </div>
  );
}

/** Shown in place of the catalogue when the CLI wasn't found. Navigates via
 *  `navigateActive`, since a portalled popover has no router to `useNavigate`. */
function NotInstalled({ label, onSettings }: { label: string; onSettings: () => void }) {
  return (
    <div className="px-2 py-3">
      <div className="text-xs text-foreground">{label} is not installed</div>
      <p className="mt-1 text-[11px] leading-relaxed text-muted-foreground">
        Chat runs the CLI from this machine, so there is nothing to send to until it is
        on your PATH.
      </p>
      <Button variant="outline" size="xs" className="mt-2.5" onClick={onSettings}>
        Open Settings → Agents
      </Button>
    </div>
  );
}

/** An empty catalogue with the provider's own reason and a way into the
 *  Settings page that fixes it. */
function EmptyNote({
  note,
  page,
  onSettings,
}: {
  note: string;
  page: SettingsPageId;
  onSettings: (page: SettingsPageId) => void;
}) {
  return (
    <div className="px-2 py-3">
      <p className="text-[11px] leading-relaxed text-muted-foreground">{note}</p>
      <Button variant="outline" size="xs" className="mt-2.5" onClick={() => onSettings(page)}>
        Open Settings → {settingsPage(page).label}
      </Button>
    </div>
  );
}

/** The starred models across agents, in starring order. A star on an agent
 *  this chat cannot move to, or whose CLI is missing, is dimmed and says why. */
function StarredList({
  stars,
  providers,
  provider,
  model,
  pending,
  locked,
  onPick,
  onUnstar,
}: {
  stars: ModelStar[];
  providers: PickerProvider[];
  provider: Provider;
  model: string | null;
  pending: { provider: Provider; model: string } | null;
  locked?: (provider: Provider) => boolean;
  onPick: (star: ModelStar) => void;
  onUnstar: (star: ModelStar) => void;
}) {
  if (stars.length === 0) {
    return (
      <div className="px-2 py-3 text-[11px] leading-relaxed text-muted-foreground">
        Star a model on an agent's tab to keep it here.
      </div>
    );
  }
  return (
    <>
      <SectionLabel>Starred</SectionLabel>
      {stars.map((star) => {
        const p = providers.find((x) => x.id === star.provider);
        const gone = !p || p.health === "missing";
        const shut = star.provider !== provider && !!locked?.(star.provider);
        const current = pending
          ? pending.provider === star.provider && pending.model === star.model
          : star.provider === provider && star.model === model;
        const agent = p?.label ?? providerLabel(star.provider);
        return (
          <ModelRow
            key={`${star.provider}:${star.model}`}
            label={p?.models.find((m) => m.id === star.model)?.label ?? star.label}
            title={[agent, star.model, shut && SWITCH_LOCKED, gone && "not installed"].filter(Boolean).join(" · ")}
            mark={<ProviderMark provider={star.provider} className="size-3.5 shrink-0" />}
            dim={gone || shut}
            selected={current}
            starred
            onStar={() => onUnstar(star)}
            onClick={() => onPick(star)}
          />
        );
      })}
    </>
  );
}

function ModelRow({
  label,
  description,
  title,
  mark,
  dim = false,
  selected,
  starred,
  onStar,
  onClick,
}: {
  label: string;
  description?: string;
  title?: string;
  /** The agent's mark, in the Starred tab where rows mix agents. */
  mark?: ReactNode;
  dim?: boolean;
  selected: boolean;
  starred: boolean;
  onStar: () => void;
  onClick: () => void;
}) {
  return (
    <div className="group/row flex w-full items-center rounded-md transition-colors hover:bg-accent has-[:focus-visible]:bg-accent">
      <button
        type="button"
        title={title ?? description}
        onClick={onClick}
        className={cn(
          "flex min-w-0 flex-1 cursor-default select-none items-center gap-2 py-1.5 pl-2 text-left text-xs text-foreground outline-none",
          dim && "opacity-50",
        )}
      >
        {mark}
        <span className="min-w-0 truncate">{label}</span>
        <Check
          className={cn("ml-auto size-3.5 shrink-0 text-muted-foreground", selected ? "opacity-100" : "opacity-0")}
        />
      </button>
      <button
        type="button"
        aria-label={starred ? `Unstar ${label}` : `Star ${label}`}
        title={starred ? "Unstar" : "Star"}
        aria-pressed={starred}
        onMouseDown={(e) => e.preventDefault()}
        onClick={onStar}
        className={cn(
          "flex size-7 shrink-0 cursor-default items-center justify-center rounded-md outline-none transition-opacity focus-visible:opacity-100",
          starred
            ? "text-warning"
            : "text-muted-foreground opacity-0 hover:text-foreground group-hover/row:opacity-100",
        )}
      >
        <Star className="size-3.5" weight={starred ? "fill" : "regular"} />
      </button>
    </div>
  );
}

function SectionLabel({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div
      className={cn(
        "sticky top-0 z-10 bg-popover px-2 pb-1 pt-2 text-[11px] font-medium text-muted-foreground",
        className,
      )}
    >
      {children}
    </div>
  );
}
