import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { CaretDown, Check, MagnifyingGlass } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { ProviderMark } from "@/components/harness/ProviderMark";
import { reasoningLabel, type HarnessModel, type Provider } from "@/lib/harness";
import { cn } from "@/lib/utils";

/** Above this many models the menu grows a search box, as bb's does. */
const SEARCH_THRESHOLD = 8;

export interface PickerProvider {
  id: Provider;
  label: string;
  models: HarnessModel[];
  /** The list is still being fetched — Codex asks its own CLI for one. */
  loading?: boolean;
}

/**
 * The composer's model-and-reasoning switcher, ported from bb's
 * `apps/app/src/components/pickers/ModelReasoningPicker.tsx`.
 *
 * The trigger reads `[mark] Model Name Level ⌄`; the menu is a strip of
 * provider tabs (marks, underlined when active), the models of the active
 * provider, and a row of reasoning levels under a rule. Picking a model
 * closes the menu, picking a level does not — the two are usually set
 * together, and the level is the fine adjustment after the coarse one.
 *
 * There is no "default" on either row. A turn always names a model and a
 * level, so what the composer shows is what the CLI is told — nothing is
 * decided out of sight by whatever the agent happens to be configured with.
 * The picked model's own default level is what a fresh pick lands on.
 *
 * Levels come off the *model*, not the provider: `claude --effort` takes
 * five, while Codex declares its own per model.
 */
export function ModelPicker({
  providers,
  provider,
  providerLocked,
  model,
  reasoning,
  onProvider,
  onModel,
  onReasoning,
  className,
}: {
  providers: PickerProvider[];
  provider: Provider;
  /** An open thread keeps its agent; only a new one may switch tabs. */
  providerLocked: boolean;
  model: string | null;
  reasoning: string | null;
  onProvider: (p: Provider) => void;
  onModel: (m: string | null) => void;
  onReasoning: (level: string | null) => void;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);

  const active = providers.find((p) => p.id === provider) ?? providers[0];
  const models = active?.models ?? [];
  const selected = models.find((m) => m.id === model) ?? null;

  // A stale search would silently hide rows the next time the menu opens.
  useEffect(() => {
    if (!open) setQuery("");
  }, [open]);

  const showSearch = models.length > SEARCH_THRESHOLD;
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return models;
    return models.filter(
      (m) => m.label.toLowerCase().includes(q) || m.id.toLowerCase().includes(q),
    );
  }, [models, query]);

  // Levels come off the picked model. The only moment without one is the beat
  // before a provider's list arrives, and the row is hidden until it does.
  const levels = selected?.reasoningEfforts ?? [];
  const level = reasoning && levels.includes(reasoning) ? reasoning : null;

  // A model id with no row is one this build does not know — a Codex model
  // added since, or a thread from an older list. Show the id rather than
  // pretending nothing is selected.
  const label = selected?.label ?? model;
  const levelText = level ? reasoningLabel(level) : null;
  const title = [active?.label, selected?.id ?? model, levelText && `${levelText} reasoning`]
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
      <ProviderMark provider={provider} className="size-3.5 shrink-0" />
      <span className="min-w-0 truncate">{label ?? "Choose a model"}</span>
      {levelText ? <span className="shrink-0 text-muted-foreground">{levelText}</span> : null}
      <CaretDown className="size-3 shrink-0 text-muted-foreground" />
    </Button>
  );

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>{trigger}</PopoverTrigger>
      <PopoverContent
        align="start"
        sideOffset={6}
        className="flex max-h-[min(var(--radix-popover-content-available-height),28rem)] w-72 flex-col overflow-hidden p-0"
        onOpenAutoFocus={(e) => {
          if (!showSearch) return;
          e.preventDefault();
          searchRef.current?.focus();
        }}
      >
        {providers.length > 1 ? (
          <div className="flex shrink-0 items-center gap-0.5 border-b border-border bg-surface px-2.5 pt-1">
            {providers.map((p) => {
              const isActive = p.id === provider;
              return (
                <button
                  key={p.id}
                  type="button"
                  title={providerLocked && !isActive ? `${p.label} — start a new chat to switch` : p.label}
                  disabled={providerLocked && !isActive}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => {
                    if (p.id !== provider) onProvider(p.id);
                  }}
                  className={cn(
                    "flex size-8 items-center justify-center border-b-2 transition-colors focus-visible:outline-none disabled:opacity-40",
                    isActive
                      ? "border-foreground text-foreground"
                      : "border-transparent text-muted-foreground enabled:hover:text-foreground",
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

        <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-1 pb-1">
          <SectionLabel>Model</SectionLabel>

          {active?.loading ? (
            <div className="px-2 py-1.5 text-xs text-muted-foreground">Loading models…</div>
          ) : filtered.length === 0 ? (
            <div className="px-2 py-1.5 text-xs text-muted-foreground">
              {query ? "No models match your search" : "No models available"}
            </div>
          ) : (
            filtered.map((m) => (
              <ModelRow
                key={m.id}
                label={m.label}
                description={m.description}
                title={m.id}
                selected={m.id === model}
                onClick={() => {
                  onModel(m.id);
                  // A level the new model does not take would be rejected by
                  // the CLI, so it falls back to that model's own default.
                  if (!reasoning || !m.reasoningEfforts.includes(reasoning)) {
                    onReasoning(m.defaultReasoningEffort ?? m.reasoningEfforts[0] ?? null);
                  }
                  setOpen(false);
                }}
              />
            ))
          )}
        </div>

        {levels.length > 0 ? (
          <>
            <div className="shrink-0 border-t border-border" />
            <div className="shrink-0 px-2 py-2.5">
              <SectionLabel className="mb-2 px-1 py-0">Reasoning</SectionLabel>
              <ToggleGroup
                type="single"
                spacing={1}
                aria-label="Reasoning"
                value={level ?? ""}
                onValueChange={(v) => {
                  // Radix clears the value when the on item is clicked again;
                  // a level stays chosen until another replaces it.
                  if (!v) return;
                  onReasoning(v);
                }}
                className="flex w-full"
              >
                {levels.map((l) => {
                  const text = reasoningLabel(l);
                  return (
                    <ToggleGroupItem
                      key={l}
                      value={l}
                      aria-label={text}
                      className="h-6 min-w-0 flex-auto shrink-0 whitespace-nowrap rounded-md px-1 text-[11px] font-normal text-muted-foreground shadow-none transition-colors hover:text-foreground data-[state=on]:bg-accent data-[state=on]:text-foreground"
                    >
                      {text}
                    </ToggleGroupItem>
                  );
                })}
              </ToggleGroup>
            </div>
          </>
        ) : null}
      </PopoverContent>
    </Popover>
  );
}

function ModelRow({
  label,
  description,
  title,
  selected,
  onClick,
}: {
  label: string;
  description?: string;
  title?: string;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      title={title ?? description}
      onClick={onClick}
      className="flex w-full cursor-default select-none items-center justify-between gap-3 rounded-md px-2 py-1.5 text-left text-xs text-foreground outline-none transition-colors hover:bg-accent focus-visible:bg-accent"
    >
      <span className="min-w-0 truncate">{label}</span>
      <Check
        className={cn("size-3.5 shrink-0 text-muted-foreground", selected ? "opacity-100" : "opacity-0")}
      />
    </button>
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
