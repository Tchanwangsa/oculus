import {
  providerAvailability,
  providerLabel,
  reasoningLabel,
  type PickerProvider,
  type Provider,
} from "@/lib/harness";

/**
 * How a selection reads against the catalogue: the chosen model's row, the
 * levels it offers and the one in force, and a title naming agent, model and
 * level. The model picker's trigger, its menu and the compact composer's
 * settings button all read it, so they name a selection the same way.
 */
export function modelSummary({
  providers,
  provider,
  model,
  reasoning,
  switchFrom,
}: {
  providers: PickerProvider[];
  provider: Provider;
  model: string | null;
  reasoning: string | null;
  switchFrom?: Provider | null;
}) {
  const active = providers.find((p) => p.id === provider);
  const availability = providerAvailability(provider);
  const unsupported = !availability.supported;
  const missing = active?.health === "missing";
  const models = unsupported ? [] : active?.models ?? [];
  const selected = models.find((m) => m.id === model) ?? null;
  const offered = unsupported || missing ? [] : selected?.reasoningEfforts ?? [];
  const level = reasoning && offered.includes(reasoning) ? reasoning : null;
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
  return { active, availability, unsupported, missing, models, selected, offered, level, label, levelText, title };
}
