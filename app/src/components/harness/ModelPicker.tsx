import { memo, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { CaretDown, Check, MagnifyingGlass } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { ProviderMark } from "@/components/harness/ProviderMark";
import { reasoningLabel, type PickerProvider, type Provider } from "@/lib/harness";
import { settingsPage, type SettingsPageId } from "@/lib/settingsSearch";
import { navigateActive } from "@/lib/tabRouters";
import { cn } from "@/lib/utils";

/** Above this many models the menu grows a search box. */
const SEARCH_THRESHOLD = 8;

export type { PickerProvider } from "@/lib/harness";

/**
 * The composer's model-and-reasoning switcher (after bb's
 * `ModelReasoningPicker`): provider tabs, the active provider's models, and
 * reasoning levels. Nothing inside closes it — agent, model and level are one
 * decision. Neither row has a "default" entry: a turn always names an explicit
 * model and level. Levels come off the model, not the provider. A provider
 * whose CLI is missing shows `NotInstalled` instead of its catalogue. Stable
 * selection props keep composer draft keystrokes outside this catalogue's
 * memo boundary.
 */
export const ModelPicker = memo(function ModelPicker({
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

  const missing = active?.health === "missing";

  const showSearch = !missing && models.length > SEARCH_THRESHOLD;
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return models;
    return models.filter(
      (m) => m.label.toLowerCase().includes(q) || m.id.toLowerCase().includes(q),
    );
  }, [models, query]);

  const levels = missing ? [] : selected?.reasoningEfforts ?? [];
  const level = reasoning && levels.includes(reasoning) ? reasoning : null;

  // An id with no row (a model this list doesn't know) shows as the raw id.
  const label = selected?.label ?? model;
  const levelText = level ? reasoningLabel(level) : null;
  const title = [
    active?.label,
    selected?.id ?? model,
    levelText && `${levelText} reasoning`,
    missing && "not installed",
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
              // A missing agent is dimmed but clickable: its panel says why.
              const gone = p.health === "missing";
              return (
                <button
                  key={p.id}
                  type="button"
                  title={
                    providerLocked && !isActive
                      ? `${p.label} — start a new chat to switch`
                      : gone
                        ? `${p.label} — not installed`
                        : p.label
                  }
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
                    gone && !isActive && "opacity-40",
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
          {missing ? (
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
                  // catalogue filters (`app/src/lib/opencodeCatalogue.ts`).
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
                    onClick={() => {
                      onModel(m.id);
                      // The CLI rejects a level the new model doesn't take.
                      if (!reasoning || !m.reasoningEfforts.includes(reasoning)) {
                        onReasoning(m.defaultReasoningEffort ?? m.reasoningEfforts[0] ?? null);
                      }
                    }}
                  />
                ))
              )}
            </>
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
                  // Radix clears on re-click; a level stays chosen.
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
});

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
